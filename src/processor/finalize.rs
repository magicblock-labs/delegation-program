use crate::error::DlpError;
use crate::processor::utils::loaders::{
    is_uninitialized_account, load_initialized_commit_record, load_initialized_commit_state,
    load_initialized_delegation_metadata, load_initialized_delegation_record,
    load_initialized_validator_fees_vault, load_owned_pda, load_program, load_signer,
};
use crate::processor::utils::pda::close_pda;
use crate::processor::utils::verify::verify_state;
use crate::state::{CommitRecord, DelegationMetadata, DelegationRecord};
use solana_program::program_error::ProgramError;
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, msg, pubkey::Pubkey, system_program,
};

/// Finalize a committed state, after validation, to a delegated account
///
/// Accounts:
///
/// 0: `[signer]`   the validator account
/// 1: `[writable]` the delegated account
/// 2: `[writable]` the commit state account
/// 3: `[writable]` the commit record account
/// 4: `[writable]` the delegation record account
/// 5: `[writable]` the delegation metadata account
/// 6: `[writable]` the validator fees vault account
/// 7: `[]`         the system program
///
/// Requirements:
///
/// - delegated account is owned by delegation program
/// - delegation record is initialized
/// - delegation metadata is initialized
/// - validator fees vault is initialized
/// - commit state is initialized and derived from the delegated account key
/// - commit record is initialized and derived from the delegated account key
/// - account mentioned in commit record is the same as the delegated account
/// - identity mentioned in commit record is the same as the validator
///
/// NOTE: that if neither commit state nor commit record are as required then
///       we skip the finalize without an error in order to not affect other finalize
///       instructions that may be bundled in the same transaction.
///
/// Steps:
///
/// 1. Validate the new state
/// 2. If the state is valid, copy the committed state to the delegated account
/// 3. Close the state diff account
/// 4. Close the commit state record
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

    load_signer(validator, "validator")?;
    load_owned_pda(delegated_account, &crate::id(), "delegated account")?;
    load_initialized_delegation_record(delegated_account, delegation_record_account, true)?;
    load_initialized_delegation_metadata(delegated_account, delegation_metadata_account, true)?;
    load_initialized_validator_fees_vault(validator, validator_fees_vault, true)?;
    load_program(system_program, system_program::id(), "system program")?;
    let load_cs = load_initialized_commit_state(delegated_account, commit_state_account, true);
    let load_cr = load_initialized_commit_record(delegated_account, commit_record_account, true);

    // Since finalize instructions are typically bundled, we return without error
    // if there is nothing to be finalized, so that correct finalizes are executed
    if let (Err(ProgramError::InvalidAccountOwner), Err(ProgramError::InvalidAccountOwner)) =
        (&load_cs, &load_cr)
    {
        if is_uninitialized_account(commit_state_account)
            && is_uninitialized_account(commit_record_account)
        {
            msg!("No state to be finalized. Skipping finalize.");
            return Ok(());
        }
    }
    load_cs?;
    load_cr?;

    // Load delegation metadata
    let mut delegation_metadata_data = delegation_metadata_account.try_borrow_mut_data()?;
    let mut delegation_metadata =
        DelegationMetadata::try_from_bytes_with_discriminator(&delegation_metadata_data)?;

    // Load delegation record
    // TODO: @@@ we borrow mut here but never modify the data, thus delegation_record
    //       does not need to be writable
    //       Modify also ../instruction_builder/finalize.rs line 28
    let mut delegation_record_data = delegation_record_account.try_borrow_mut_data()?;
    let delegation_record =
        DelegationRecord::try_from_bytes_with_discriminator_mut(&mut delegation_record_data)?;

    // Load commit record
    let commit_record_data = commit_record_account.try_borrow_data()?;
    let commit_record = CommitRecord::try_from_bytes_with_discriminator(&commit_record_data)?;

    verify_state(
        validator,
        delegation_record,
        commit_record,
        commit_state_account,
    )?;

    // Check that the commit record is the right one
    if !commit_record.account.eq(delegated_account.key) {
        return Err(DlpError::InvalidDelegatedAccount.into());
    }
    if !commit_record.identity.eq(validator.key) {
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
    delegation_metadata.last_update_external_slot = commit_record.slot;
    delegation_metadata.to_bytes_with_discriminator(&mut delegation_metadata_data.as_mut())?;

    // Update the delegation record
    delegation_record.lamports = delegated_account.lamports();

    // Load commit state
    let commit_state_data = commit_state_account.try_borrow_data()?;

    // Copying the new commit state to the delegated account
    delegated_account.realloc(commit_state_data.len(), false)?;
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
fn settle_lamports_balance<'a, 'info>(
    delegated_account: &'a AccountInfo<'info>,
    commit_state_account: &'a AccountInfo<'info>,
    validator_fees_vault: &'a AccountInfo<'info>,
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

    **transfer_source.try_borrow_mut_lamports()? = transfer_source
        .lamports()
        .checked_sub(transfer_lamports)
        .ok_or(DlpError::Overflow)?;
    **transfer_destination.try_borrow_mut_lamports()? = transfer_destination
        .lamports()
        .checked_add(transfer_lamports)
        .ok_or(DlpError::Overflow)?;

    Ok(())
}
