use crate::consts::ADMIN_PUBKEY;
use crate::error::DlpError::Unauthorized;
use crate::processor::utils::loaders::{load_initialized_fees_vault, load_signer};
use solana_program::program_error::ProgramError;
use solana_program::rent::Rent;
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey};

/// Process request to claim fees from the protocol fees vault
///
/// 1. Transfer lamports from protocol fees_vault PDA to the admin authority
pub fn process_protocol_claim_fees(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    _data: &[u8],
) -> ProgramResult {
    // Load Accounts
    let [admin, fees_vault] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Check if the admin is signer
    load_signer(admin)?;
    load_initialized_fees_vault(fees_vault, true)?;

    // Check if the admin is the correct one
    if !admin.key.eq(&ADMIN_PUBKEY) {
        return Err(Unauthorized.into());
    }

    // Calculate the amount to transfer
    let min_rent = Rent::default().minimum_balance(8);
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
