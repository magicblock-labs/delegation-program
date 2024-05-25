use solana_program::program_error::ProgramError;
use solana_program::rent::Rent;
use solana_program::sysvar::Sysvar;
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    pubkey::Pubkey,
    system_program, {self},
};

use crate::loaders::{load_owned_pda, load_program, load_signer};
use crate::state::{CommitRecord, DelegationRecord};
use crate::utils::close_pda;
use crate::utils_account::AccountDeserialize;
use crate::verify_state::verify_state;

/// Finalize a committed state, after validation, to a delegated account
///
/// 1. Validate the new state
/// 2. If the state is valid, copy the committed state to the delegated account
/// 3. Close the state diff account
/// 3. Close the commit state record
///
///
/// Accounts expected: Authority Record, Buffer PDA, Delegated PDA
pub fn process_finalize(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    _data: &[u8],
) -> ProgramResult {
    let [payer, delegated_account, committed_state_account, committed_state_record, delegation_record, reimbursement, system_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    load_signer(payer)?;
    load_owned_pda(delegated_account, &crate::id())?;
    load_owned_pda(committed_state_account, &crate::id())?;
    load_owned_pda(committed_state_record, &crate::id())?;
    load_owned_pda(delegation_record, &crate::id())?;
    load_program(system_program, system_program::id())?;

    // Load delegation record
    let mut delegation_data = delegation_record.try_borrow_mut_data()?;
    let delegation = DelegationRecord::try_from_bytes_mut(&mut delegation_data)?;

    // Load committed state
    let commit_record_data = committed_state_record.try_borrow_data()?;
    let commit_record = CommitRecord::try_from_bytes(&commit_record_data)?;

    verify_state(delegation, commit_record, committed_state_account)?;

    if !commit_record.account.eq(delegated_account.key) {
        msg!("Delegated account does not match the expected account");
        return Err(ProgramError::InvalidAccountData);
    }

    if !commit_record.identity.eq(reimbursement.key) {
        msg!("Reimbursement account does not match the expected account");
        return Err(ProgramError::InvalidAccountData);
    }

    let new_data = committed_state_account.try_borrow_data()?;

    // Make it rent exempt
    resize_and_rent_exempt(delegated_account, committed_state_account, new_data.len())?;

    // Copying the new state to the delegated account
    delegated_account.realloc(new_data.len(), false)?;
    let mut delegated_account_data = delegated_account.try_borrow_mut_data()?;
    (*delegated_account_data).copy_from_slice(&new_data);

    // Dropping references
    drop(delegated_account_data);
    drop(commit_record_data);
    drop(new_data);

    // Closing accounts
    close_pda(committed_state_record, reimbursement)?;
    close_pda(committed_state_account, reimbursement)?;
    Ok(())
}

/// Resize the account to hold data_size and make it rent exempt,
/// subtracting the rent_exempt_balance from the payer
fn resize_and_rent_exempt(
    target_account: &AccountInfo,
    payer: &AccountInfo,
    data_size: usize,
) -> Result<(), ProgramError> {
    let rent_exempt_balance = Rent::get()?
        .minimum_balance(data_size)
        .saturating_sub(target_account.lamports());
    if rent_exempt_balance.gt(&0) {
        let new_lamports = payer
            .try_borrow_mut_lamports()?
            .checked_sub(rent_exempt_balance)
            .ok_or(ProgramError::InvalidAccountData)?;
        **payer.try_borrow_mut_lamports()? = new_lamports;

        let delegated_lamports = target_account
            .try_borrow_mut_lamports()?
            .checked_add(rent_exempt_balance)
            .ok_or(ProgramError::InvalidAccountData)?;
        **target_account.try_borrow_mut_lamports()? = delegated_lamports;
    }
    Ok(())
}
