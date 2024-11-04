use crate::consts::VALIDATOR_FEES_VAULT;
use crate::error::DlpError;
use crate::pda::validator_fees_vault_pda_from_pubkey;
use crate::state::{CommitRecord, DelegationMetadata, DelegationRecord};
use crate::utils::balance_lamports::settle_lamports_balance;
use crate::utils::loaders::{load_initialized_pda, load_owned_pda, load_program, load_signer};
use crate::utils::utils_account::AccountDeserialize;
use crate::utils::utils_pda::close_pda;
use crate::utils::verify_state::verify_state;
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
/// 3. Close the commit state record
///
///
/// Accounts expected: Authority Record, Buffer PDA, Delegated PDA
pub fn process_finalize(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    _data: &[u8],
) -> ProgramResult {
    let [validator, delegated_account, committed_state_account, committed_state_record, delegation_record, delegation_metadata, reimbursement, validator_fees_vault, system_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    load_signer(validator)?;
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

    // Check that the validator fees vault account is correct and initialized
    if !validator_fees_vault_pda_from_pubkey(validator.key).eq(validator_fees_vault.key) {
        return Err(DlpError::InvalidAuthority.into());
    }
    load_initialized_pda(
        validator_fees_vault,
        &[VALIDATOR_FEES_VAULT, &validator.key.to_bytes()],
        &crate::id(),
        true,
    )?;

    // If the commit slot is greater than the last update slot, we verify and finalize the state
    // If slot is equal or less, we simply close the commitment accounts
    if commit_record.slot > delegation_metadata.last_update_external_slot {
        verify_state(
            validator,
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
        settle_lamports_balance(
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
        delegation_metadata.last_update_lamports = delegated_account.lamports();
        delegation_metadata.serialize(&mut &mut delegation_metadata_data.as_mut())?;

        // Dropping references
        drop(delegated_account_data);
        drop(commit_record_data);
        drop(new_data);
    }

    // Closing accounts
    close_pda(committed_state_record, reimbursement)?;
    close_pda(committed_state_account, reimbursement)?;
    Ok(())
}
