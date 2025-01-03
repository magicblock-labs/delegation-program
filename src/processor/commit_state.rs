use crate::args::CommitStateArgs;
use crate::error::DlpError;
use crate::processor::utils::loaders::{
    load_initialized_delegation_metadata, load_initialized_delegation_record,
    load_initialized_validator_fees_vault, load_owned_pda, load_program, load_program_config,
    load_signer, load_uninitialized_pda,
};
use crate::processor::utils::pda::create_pda;
use crate::state::{CommitRecord, DelegationMetadata, DelegationRecord, ProgramConfig};
use crate::{
    commit_record_seeds_from_delegated_account, commit_state_seeds_from_delegated_account,
};
use borsh::BorshDeserialize;
use solana_program::program::invoke;
use solana_program::program_error::ProgramError;
use solana_program::system_instruction::transfer;
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey};
use solana_program::{msg, system_program};

/// Commit a new state of a delegated Pda
///
/// 1. Check that the pda is delegated
/// 2. Init a new PDA to store the new state
/// 3. Copy the new state to the new PDA
/// 4. Init a new PDA to store the record of the new state commitment
///
pub fn process_commit_state(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let args = CommitStateArgs::try_from_slice(data)?;

    let commit_state_bytes: &[u8] = args.data.as_ref();
    let commit_record_lamports = args.lamports;
    let commit_record_slot = args.slot;

    let [validator, delegated_account, commit_state_account, commit_record_account, delegation_record_account, delegation_metadata_account, validator_fees_vault, program_config_account, system_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Check that the origin account is delegated
    load_owned_pda(delegated_account, &crate::id())?;
    load_signer(validator)?;
    load_initialized_delegation_record(delegated_account, delegation_record_account, false)?;
    load_initialized_delegation_metadata(delegated_account, delegation_metadata_account, true)?;
    load_initialized_validator_fees_vault(validator, validator_fees_vault, false)?;
    load_program(system_program, system_program::id())?;

    // Read delegation metadata
    let mut delegation_metadata_data = delegation_metadata_account.try_borrow_mut_data()?;
    let mut delegation_metadata =
        DelegationMetadata::try_from_bytes_with_discriminator(&delegation_metadata_data)?;

    // If the commit slot is greater than the last update slot, we can proceed.
    // If the slot is equal or less, we simply do not commit.
    // Since commit instructions are typically bundled, we return without error
    // so that correct commits are executed.
    if commit_record_slot <= delegation_metadata.last_update_external_slot {
        msg!(
            "Slot {} is outdated, previous slot is {}. Skipping commit",
            commit_record_slot,
            delegation_metadata.last_update_external_slot
        );
        return Ok(());
    }

    // Once the account is marked as undelegatable, any subsequent commit should fail
    if delegation_metadata.is_undelegatable {
        return Err(DlpError::AlreadyUndelegated.into());
    }

    // Update delegation metadata undelegation flag
    delegation_metadata.is_undelegatable = args.allow_undelegation;
    delegation_metadata.to_bytes_with_discriminator(&mut delegation_metadata_data.as_mut())?;

    // Load delegation record
    let delegation_record_data = delegation_record_account.try_borrow_data()?;
    let delegation_record =
        DelegationRecord::try_from_bytes_with_discriminator(&delegation_record_data)?;

    // If there was an issue with the lamport accounting in the past, abort (this should never happen)
    if delegated_account.lamports() < delegation_record.lamports {
        return Err(DlpError::InvalidDelegatedState.into());
    }

    // If committed lamports are more than the previous lamports balance, deposit the difference in the commitment account
    // If committed lamports are less than the previous lamports balance, we have collateral to settle the balance at state finalization
    // We need to do that so that the finalizer already have all the lamports from the validators ready at finalize time
    // The finalizer can return any extra lamport to the validator during finalize, but this acts as the validator's proof of collateral
    if commit_record_lamports > delegation_record.lamports {
        let extra_lamports = commit_record_lamports
            .checked_sub(delegation_record.lamports)
            .ok_or(DlpError::Overflow)?;
        invoke(
            &transfer(validator.key, commit_state_account.key, extra_lamports),
            &[
                validator.clone(),
                commit_state_account.clone(),
                system_program.clone(),
            ],
        )?;
    }

    // Load the program configuration and validate it, if any
    let has_program_config =
        load_program_config(program_config_account, delegation_record.owner, false)?;
    if has_program_config {
        let program_config_data = program_config_account.try_borrow_data()?;
        let program_config =
            ProgramConfig::try_from_bytes_with_discriminator(&program_config_data)?;
        if !program_config.approved_validators.contains(validator.key) {
            return Err(DlpError::InvalidWhitelistProgramConfig.into());
        }
    }

    // Load the uninitialized PDAs
    let commit_state_bump = load_uninitialized_pda(
        commit_state_account,
        commit_state_seeds_from_delegated_account!(delegated_account.key),
        &crate::id(),
        true,
    )?;
    let commit_record_bump = load_uninitialized_pda(
        commit_record_account,
        commit_record_seeds_from_delegated_account!(delegated_account.key),
        &crate::id(),
        true,
    )?;

    // Initialize the PDA containing the new committed state
    create_pda(
        commit_state_account,
        &crate::id(),
        commit_state_bytes.len(),
        commit_state_seeds_from_delegated_account!(delegated_account.key),
        commit_state_bump,
        system_program,
        validator,
    )?;

    // Initialize the PDA containing the record of the committed state
    create_pda(
        commit_record_account,
        &crate::id(),
        CommitRecord::size_with_discriminator(),
        commit_record_seeds_from_delegated_account!(delegated_account.key),
        commit_record_bump,
        system_program,
        validator,
    )?;

    // Initialize the commit record
    let commit_record = CommitRecord {
        identity: *validator.key,
        account: *delegated_account.key,
        slot: commit_record_slot,
        lamports: commit_record_lamports,
    };
    let mut commit_record_data = commit_record_account.try_borrow_mut_data()?;
    commit_record.to_bytes_with_discriminator(&mut commit_record_data)?;

    // Copy the new state to the initialized PDA
    let mut commit_state_data = commit_state_account.try_borrow_mut_data()?;
    (*commit_state_data).copy_from_slice(commit_state_bytes);

    // TODO - Add additional validation for the commitment, e.g. sufficient validator stake

    Ok(())
}
