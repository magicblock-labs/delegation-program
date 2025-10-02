use pinocchio::account_info::AccountInfo;
use pinocchio::program_error::ProgramError;
use pinocchio::pubkey::{pubkey_eq, Pubkey};
use pinocchio::ProgramResult;
use pinocchio_log::log;

use crate::error::DlpError;
use crate::processor::fast::utils::pda::close_pda;
use crate::processor::fast::utils::requires::{
    is_uninitialized_account, require_initialized_commit_record, require_initialized_commit_state,
    require_initialized_delegation_metadata, require_initialized_delegation_record,
    require_initialized_validator_fees_vault, require_owned_pda, require_program, require_signer,
};
use crate::state::{CommitRecord, DelegationMetadata, DelegationRecord};

use super::to_pinocchio_program_error;

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

    require_signer(validator, "validator")?;
    require_owned_pda(delegated_account, &crate::fast::ID, "delegated account")?;
    require_initialized_delegation_record(delegated_account, delegation_record_account, true)?;
    require_initialized_delegation_metadata(delegated_account, delegation_metadata_account, true)?;
    require_initialized_validator_fees_vault(validator, validator_fees_vault, true)?;
    require_program(system_program, &pinocchio_system::ID, "system program")?;

    let require_cs =
        require_initialized_commit_state(delegated_account, commit_state_account, true);
    let require_cr =
        require_initialized_commit_record(delegated_account, commit_record_account, true);

    // Since finalize instructions are typically bundled, we return without error
    // if there is nothing to be finalized, so that correct finalizes are executed
    if let (Err(ProgramError::InvalidAccountOwner), Err(ProgramError::InvalidAccountOwner)) =
        (&require_cs, &require_cr)
    {
        if is_uninitialized_account(commit_state_account)
            && is_uninitialized_account(commit_record_account)
        {
            log!("No state to be finalized. Skipping finalize.");
            return Ok(());
        }
    }
    require_cs?;
    require_cr?;

    // Load delegation metadata
    let mut delegation_metadata_data = delegation_metadata_account.try_borrow_mut_data()?;
    let mut delegation_metadata =
        DelegationMetadata::try_from_bytes_with_discriminator(&delegation_metadata_data)
            .map_err(to_pinocchio_program_error)?;

    let mut delegation_record_data = delegation_record_account.try_borrow_mut_data()?;
    let delegation_record =
        DelegationRecord::try_from_bytes_with_discriminator_mut(&mut delegation_record_data)
            .map_err(to_pinocchio_program_error)?;

    // Load commit record
    let commit_record_data = commit_record_account.try_borrow_data()?;
    let commit_record = CommitRecord::try_from_bytes_with_discriminator(&commit_record_data)
        .map_err(to_pinocchio_program_error)?;

    // Check that the commit record is the right one
    if !pubkey_eq(commit_record.account.as_array(), delegated_account.key()) {
        return Err(DlpError::InvalidDelegatedAccount.into());
    }
    if !pubkey_eq(commit_record.identity.as_array(), validator.key()) {
        return Err(DlpError::InvalidReimbursementAccount.into());
    }

    // Settle accounts lamports
    settle_lamports_balance(
        delegated_account,
        commit_state_account,
        validator_fees_vault,
        delegation_record.lamports,
        commit_record.lamports,
    )?;

    // Update the delegation metadata
    delegation_metadata.last_update_nonce = commit_record.nonce;
    delegation_metadata
        .to_bytes_with_discriminator(&mut delegation_metadata_data.as_mut())
        .map_err(to_pinocchio_program_error)?;

    // Update the delegation record
    delegation_record.lamports = delegated_account.lamports();

    // Load commit state
    let commit_state_data = commit_state_account.try_borrow_data()?;

    // Copying the new commit state to the delegated account
    delegated_account.resize(commit_state_data.len())?;
    let mut delegated_account_data = delegated_account.try_borrow_mut_data()?;
    (*delegated_account_data).copy_from_slice(&commit_state_data);

    // Drop remaining reference before closing accounts
    drop(commit_record_data);
    drop(commit_state_data);

    // Closing accounts
    close_pda(commit_state_account, validator)?;
    close_pda(commit_record_account, validator)?;

    Ok(())
}

/// Settle the committed lamports to the delegated account
fn settle_lamports_balance(
    delegated_account: &AccountInfo,
    commit_state_account: &AccountInfo,
    validator_fees_vault: &AccountInfo,
    delegation_record_lamports: u64,
    commit_record_lamports: u64,
) -> Result<(), ProgramError> {
    let (transfer_source, transfer_destination, transfer_lamports) =
        match delegation_record_lamports.cmp(&commit_record_lamports) {
            std::cmp::Ordering::Greater => (
                delegated_account,
                validator_fees_vault,
                delegation_record_lamports
                    .checked_sub(commit_record_lamports)
                    .ok_or(DlpError::Overflow)?,
            ),
            std::cmp::Ordering::Less => (
                commit_state_account,
                delegated_account,
                commit_record_lamports
                    .checked_sub(delegation_record_lamports)
                    .ok_or(DlpError::Overflow)?,
            ),
            std::cmp::Ordering::Equal => return Ok(()),
        };

    *transfer_source.try_borrow_mut_lamports()? = transfer_source
        .lamports()
        .checked_sub(transfer_lamports)
        .ok_or(DlpError::Overflow)?;
    *transfer_destination.try_borrow_mut_lamports()? = transfer_destination
        .lamports()
        .checked_add(transfer_lamports)
        .ok_or(DlpError::Overflow)?;

    Ok(())
}
