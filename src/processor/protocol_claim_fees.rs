use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program_error::ProgramError,
    pubkey::Pubkey, rent::Rent, sysvar::Sysvar,
};

use crate::processor::utils::loaders::{load_account, load_initialized_protocol_fees_vault};
use crate::state::FeesVault;

/// Process request to claim fees from the protocol fees vault
///
/// Accounts:
///
/// 1. `[writable]` protocol fees vault PDA
/// 2. `[writable]` fees receiver
///
/// Requirements:
///
/// - protocol fees vault is initialized
/// - protocol fees vault has enough lamports to claim fees and still be
///   rent exempt
/// - fees receiver is the correct one
///
/// 1. Transfer lamports from the protocol fees vault PDA to the configured fees receiver
pub fn process_protocol_claim_fees(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    _data: &[u8],
) -> ProgramResult {
    // Load Accounts
    let [fees_vault_account, fees_receiver] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Validate vault PDA and configured receiver
    load_initialized_protocol_fees_vault(fees_vault_account, true)?;

    let fees_vault_data = fees_vault_account.try_borrow_data()?;
    let fees_vault = FeesVault::try_from_bytes_with_discriminator(&fees_vault_data)?;

    load_account(
        fees_receiver,
        fees_vault.fees_receiver,
        true,
        "fees receiver",
    )?;

    if fees_receiver.key == fees_vault_account.key {
        // Nothing to transfer, or ambiguous aliasing – reject explicitly
        return Err(ProgramError::InvalidArgument);
    }

    // Calculate the amount to transfer
    let min_rent = Rent::get()?.minimum_balance(fees_vault_account.data_len());
    if fees_vault_account.lamports() < min_rent {
        return Err(ProgramError::InsufficientFunds);
    }
    let amount = fees_vault_account.lamports() - min_rent;
    if amount == 0 {
        return Ok(());
    }

    // Transfer fees to the configured fees receiver
    **fees_vault_account.try_borrow_mut_lamports()? = fees_vault_account
        .lamports()
        .checked_sub(amount)
        .ok_or(ProgramError::InsufficientFunds)?;

    **fees_receiver.try_borrow_mut_lamports()? = fees_receiver
        .lamports()
        .checked_add(amount)
        .ok_or(ProgramError::ArithmeticOverflow)?;

    Ok(())
}
