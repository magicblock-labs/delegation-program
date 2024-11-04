use crate::error::DlpError;
use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::program_error::ProgramError;
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    system_program, {self},
};

use crate::loaders::{load_owned_pda, load_program, load_signer};
use crate::state::{CommitRecord, DelegationMetadata, DelegationRecord};
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
    let [authority, delegated_account, committed_state_account, committed_state_record, delegation_record, delegation_metadata, reimbursement, validator_fees_vault, system_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    load_signer(authority)?;
    load_owned_pda(delegated_account, &crate::id())?;
    load_owned_pda(committed_state_account, &crate::id())?;
    load_owned_pda(committed_state_record, &crate::id())?;
    load_owned_pda(delegation_record, &crate::id())?;
    load_owned_pda(delegation_metadata, &crate::id())?;
    load_program(system_program, system_program::id())?;

    // Load delegation record
    let mut delegation_data = delegation_record.try_borrow_mut_data()?;
    let delegation = DelegationRecord::try_from_bytes_mut(&mut delegation_data)?;

    // Load delegation metadata
    let mut delegation_metadata_data = delegation_metadata.try_borrow_mut_data()?;
    let mut delegation_metadata = DelegationMetadata::try_from_slice(&delegation_metadata_data)?;

    // Load committed state
    let commit_record_data = committed_state_record.try_borrow_data()?;
    let commit_record = CommitRecord::try_from_bytes(&commit_record_data)?;

    verify_state(
        authority,
        delegation,
        commit_record,
        committed_state_account,
    )?;

    if !commit_record.account.eq(delegated_account.key) {
        return Err(DlpError::InvalidDelegatedAccount.into());
    }

    if !commit_record.identity.eq(reimbursement.key) {
        return Err(DlpError::InvalidReimbursementAccount.into());
    }

    let new_data = committed_state_account.try_borrow_data()?;

    // Balance lamports
    let lamports_difference =
        delegation_metadata.last_update_lamports as i64 - commit_record.lamports as i64;
    balance_lamports(
        delegated_account,
        committed_state_account,
        lamports_difference,
        validator_fees_vault,
    )?;

    // Copying the new state to the delegated account
    delegated_account.realloc(new_data.len(), false)?;
    let mut delegated_account_data = delegated_account.try_borrow_mut_data()?;
    (*delegated_account_data).copy_from_slice(&new_data);

    delegation_metadata.last_update_external_slot = commit_record.slot;
    delegation_metadata.serialize(&mut &mut delegation_metadata_data.as_mut())?;

    // Dropping references
    drop(delegated_account_data);
    drop(commit_record_data);
    drop(new_data);

    // Closing accounts
    close_pda(committed_state_record, reimbursement)?;
    close_pda(committed_state_account, reimbursement)?;
    Ok(())
}

/// Balance the lamports of the delegated account
fn balance_lamports(
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
