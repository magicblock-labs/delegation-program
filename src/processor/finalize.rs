use crate::error::DlpError;
use crate::processor::utils::lamports::settle_lamports_balance;
use crate::processor::utils::loaders::{
    load_initialized_commit_record, load_initialized_commit_state,
    load_initialized_delegation_metadata, load_initialized_delegation_record,
    load_initialized_validator_fees_vault, load_owned_pda, load_program, load_signer,
};
use crate::processor::utils::pda::close_pda;
use crate::processor::utils::verify::verify_state;
use crate::state::account::AccountDeserialize;
use crate::state::{CommitRecord, DelegationMetadata, DelegationRecord};
use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::program_error::ProgramError;
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    system_program, {self},
};

/// Finalize a committed state, after validation, to a delegated account
///
/// 1. Validate the new state
/// 2. If the state is valid, copy the committed state to the delegated account
/// 3. Close the state diff account
/// 4. Close the commit state record
///
///
/// Accounts expected: Authority Record, Buffer PDA, Delegated PDA
pub fn process_finalize(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    _data: &[u8],
) -> ProgramResult {
    let [validator, delegated_account, commit_state_account, commit_record_account, delegation_record_account, delegation_metadata_account, validator_fees_vault, system_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    load_signer(validator)?;
    load_owned_pda(delegated_account, &crate::id())?;
    load_initialized_commit_state(delegated_account, commit_state_account, true)?;
    load_initialized_commit_record(delegated_account, commit_record_account, true)?;
    load_initialized_delegation_record(delegated_account, delegation_record_account, true)?;
    load_initialized_delegation_metadata(delegated_account, delegation_metadata_account, true)?;
    load_initialized_validator_fees_vault(validator, validator_fees_vault, true)?;
    load_program(system_program, system_program::id())?;

    // Load delegation record
    let mut delegation_record_data = delegation_record_account.try_borrow_mut_data()?;
    let delegation_record = DelegationRecord::try_from_bytes_mut(&mut delegation_record_data)?;

    // Load delegation metadata
    let mut delegation_metadata_data = delegation_metadata_account.try_borrow_mut_data()?;
    let mut delegation_metadata = DelegationMetadata::try_from_slice(&delegation_metadata_data)?;

    // Load committed state
    let commit_record_data = commit_record_account.try_borrow_data()?;
    let commit_record = CommitRecord::try_from_bytes(&commit_record_data)?;

    // If the commit slot is greater than the last update slot, we verify and finalize the state
    // If slot is equal or less, we simply close the commitment accounts
    if commit_record.slot > delegation_metadata.last_update_external_slot {
        verify_state(
            validator,
            delegation_record,
            commit_record,
            commit_state_account,
        )?;

        if !commit_record.account.eq(delegated_account.key) {
            return Err(DlpError::InvalidDelegatedAccount.into());
        }

        if !commit_record.identity.eq(validator.key) {
            return Err(DlpError::InvalidReimbursementAccount.into());
        }

        let commit_state_data = commit_state_account.try_borrow_data()?;

        // Balance lamports
        let lamports_difference = delegation_record.lamports as i64 - commit_record.lamports as i64;
        settle_lamports_balance(
            delegated_account,
            commit_state_account,
            lamports_difference,
            validator_fees_vault,
        )?;

        // Copying the new state to the delegated account
        delegated_account.realloc(commit_state_data.len(), false)?;
        let mut delegated_account_data = delegated_account.try_borrow_mut_data()?;
        (*delegated_account_data).copy_from_slice(&commit_state_data);

        delegation_metadata.last_update_external_slot = commit_record.slot;
        delegation_record.lamports = delegated_account.lamports();
        delegation_metadata.serialize(&mut &mut delegation_metadata_data.as_mut())?;

        // Dropping references
        drop(delegated_account_data);
        drop(commit_record_data);
        drop(commit_state_data);
    }

    // Closing accounts
    close_pda(commit_record_account, validator)?;
    close_pda(commit_state_account, validator)?;
    Ok(())
}
