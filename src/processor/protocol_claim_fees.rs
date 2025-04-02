use crate::error::DlpError::Unauthorized;
use crate::processor::utils::loaders::{
    load_initialized_protocol_fees_vault, load_program_upgrade_authority, load_signer,
};
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
    let [admin, fees_vault, delegation_program_data] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Check if the admin is signer
    load_signer(admin, "admin")?;
    load_initialized_protocol_fees_vault(fees_vault, true)?;

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

    **admin.try_borrow_mut_lamports()? = admin
        .lamports()
        .checked_add(amount)
        .ok_or(ProgramError::ArithmeticOverflow)?;

    Ok(())
}
