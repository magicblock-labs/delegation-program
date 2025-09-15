use crate::error::DlpError::Unauthorized;
use crate::processor::utils::loaders::{
    load_account, load_initialized_protocol_fees_vault, load_program_config,
    load_program_upgrade_authority, load_signer,
};
use crate::state::ProgramConfig;
use solana_program::msg;
use solana_program::program_error::ProgramError;
use solana_program::rent::Rent;
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey};

/// Process request to claim fees from the protocol fees vault
///
/// Accounts:
///
/// 1. `[signer]`   admin account that can claim the fees
/// 2. `[writable]` protocol fees vault PDA
/// 3. `[]` program config PDA
/// 4. `[writable]` fees receiver PDA
/// 5. `[]`         delegation program
///
/// Requirements:
///
/// - protocol fees vault is initialized
/// - protocol fees vault has enough lamports to claim fees and still be
///   rent exempt
/// - admin is the protocol fees vault admin
///
/// 1. Transfer lamports from protocol fees_vault PDA to the admin authority
pub fn process_protocol_claim_fees(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    _data: &[u8],
) -> ProgramResult {
    // Load Accounts
    let [admin, fees_vault, program_config_account, fees_receiver, program, delegation_program_data] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Check if the admin is signer
    load_signer(admin, "admin")?;
    load_initialized_protocol_fees_vault(fees_vault, true)?;
    load_program_config(program_config_account, *program.key, true)?;

    // Check if the admin is the correct one
    let admin_pubkey =
        load_program_upgrade_authority(&crate::ID, delegation_program_data)?.ok_or(Unauthorized)?;
    if !admin.key.eq(&admin_pubkey) {
        msg!(
            "Expected admin pubkey: {} but got {}",
            admin_pubkey,
            admin.key
        );
        return Err(Unauthorized.into());
    }

    let program_config_data = program_config_account.try_borrow_data()?;
    let program_config = ProgramConfig::try_from_bytes_with_discriminator(&program_config_data)?;

    load_account(
        fees_receiver,
        program_config.fees_receiver,
        true,
        "fees receiver",
    )?;

    // Calculate the amount to transfer
    let min_rent = Rent::default().minimum_balance(8);
    if fees_vault.lamports() < min_rent {
        return Err(ProgramError::InsufficientFunds);
    }
    let amount = fees_vault.lamports() - min_rent;

    // Transfer fees to the admin pubkey
    **fees_vault.try_borrow_mut_lamports()? = fees_vault
        .lamports()
        .checked_sub(amount)
        .ok_or(ProgramError::InsufficientFunds)?;

    **fees_receiver.try_borrow_mut_lamports()? = fees_receiver
        .lamports()
        .checked_add(amount)
        .ok_or(ProgramError::ArithmeticOverflow)?;

    Ok(())
}
