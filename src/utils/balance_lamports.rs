use solana_program::account_info::AccountInfo;
use solana_program::program_error::ProgramError;

/// Settle the committed lamports to the delegated account
pub(crate) fn settle_lamports_balance(
    target_account: &AccountInfo,
    commited_state_account: &AccountInfo,
    lamports_difference: i64,
    validator_fees_vault: &AccountInfo,
) -> Result<(), ProgramError> {
    // If the lamports difference is positive, we transfer the lamports from the target account to the validator fees vault
    if lamports_difference > 0 {
        let new_lamports = target_account
            .try_borrow_lamports()?
            .checked_sub(lamports_difference.unsigned_abs())
            .ok_or(ProgramError::InvalidAccountData)?;
        **target_account.try_borrow_mut_lamports()? = new_lamports;
        let new_lamports = validator_fees_vault
            .try_borrow_lamports()?
            .checked_add(lamports_difference.unsigned_abs())
            .ok_or(ProgramError::InvalidAccountData)?;
        **validator_fees_vault.try_borrow_mut_lamports()? = new_lamports;
    }
    // If the lamports difference is negative, we transfer the lamports from the commited state account to the target account
    if lamports_difference < 0 {
        let new_lamports = target_account
            .try_borrow_lamports()?
            .checked_add(lamports_difference.unsigned_abs())
            .ok_or(ProgramError::InvalidAccountData)?;
        **target_account.try_borrow_mut_lamports()? = new_lamports;
        let new_lamports = commited_state_account
            .try_borrow_lamports()?
            .checked_sub(lamports_difference.unsigned_abs())
            .ok_or(ProgramError::InvalidAccountData)?;
        **commited_state_account.try_borrow_mut_lamports()? = new_lamports;
    }
    Ok(())
}
