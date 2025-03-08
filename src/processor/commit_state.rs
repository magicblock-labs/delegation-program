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

/// Commit a new state of a delegated PDA
///
/// Accounts:
///
/// 0: `[signer]`   the validator requesting the commit
/// 1: `[]`         the delegated account
/// 2: `[writable]` the PDA storing the new state
/// 3: `[writable]` the PDA storing the commit record
/// 4: `[]`         the delegation record
/// 5: `[writable]` the delegation metadata
/// 6: `[]`         the validator fees vault
/// 7: `[]`         the program config account
/// 8: `[]`         the system program
///
/// Requirements:
///
/// - delegation record is initialized
/// - delegation metadata is initialized
/// - validator fees vault is initialized
/// - program config is initialized
/// - commit state is uninitialized
/// - commit record is uninitialized
/// - delegated account holds at least the lamports indicated in the delegation record
/// - account was not committed at a later slot
///
/// Steps:
/// 1. Check that the pda is delegated
/// 2. Init a new PDA to store the new state
/// 3. Copy the new state to the new PDA
/// 4. Init a new PDA to store the record of the new state commitment
pub fn process_commit_state(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let args = CommitStateArgs::try_from_slice(data)?;

    let commit_state_bytes: &[u8] = args.data.as_ref();
    let commit_record_lamports = args.lamports;
    let commit_record_slot = args.slot;
    let allow_undelegation = args.allow_undelegation;

    let [validator, delegated_account, commit_state_account, commit_record_account, delegation_record_account, delegation_metadata_account, validator_fees_vault, program_config_account, system_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    let commit_args = CommitStateInternalArgs {
        commit_state_bytes,
        commit_record_lamports,
        commit_record_slot,
        allow_undelegation,
        validator,
        delegated_account,
        commit_state_account,
        commit_record_account,
        delegation_record_account,
        delegation_metadata_account,
        validator_fees_vault,
        program_config_account,
        system_program,
    };

    process_commit_state_internal(commit_args)
}

/// Arguments for the commit state internal function
pub(crate) struct CommitStateInternalArgs<'a, 'info> {
    pub(crate) commit_state_bytes: &'a [u8],
    pub(crate) commit_record_lamports: u64,
    pub(crate) commit_record_slot: u64,
    pub(crate) allow_undelegation: bool,
    pub(crate) validator: &'a AccountInfo<'info>,
    pub(crate) delegated_account: &'a AccountInfo<'info>,
    pub(crate) commit_state_account: &'a AccountInfo<'info>,
    pub(crate) commit_record_account: &'a AccountInfo<'info>,
    pub(crate) delegation_record_account: &'a AccountInfo<'info>,
    pub(crate) delegation_metadata_account: &'a AccountInfo<'info>,
    pub(crate) validator_fees_vault: &'a AccountInfo<'info>,
    pub(crate) program_config_account: &'a AccountInfo<'info>,
    pub(crate) system_program: &'a AccountInfo<'info>,
}

/// Commit a new state of a delegated Pda
pub(crate) fn process_commit_state_internal(
    args: CommitStateInternalArgs,
) -> Result<(), ProgramError> {
    // Check that the origin account is delegated
    load_owned_pda(args.delegated_account, &crate::id(), "delegated account")?;
    load_signer(args.validator, "validator account")?;
    load_initialized_delegation_record(
        args.delegated_account,
        args.delegation_record_account,
        false,
    )?;
    load_initialized_delegation_metadata(
        args.delegated_account,
        args.delegation_metadata_account,
        true,
    )?;
    load_initialized_validator_fees_vault(args.validator, args.validator_fees_vault, false)?;
    load_program(args.system_program, system_program::id(), "system program")?;

    // Read delegation metadata
    let mut delegation_metadata_data = args.delegation_metadata_account.try_borrow_mut_data()?;
    let mut delegation_metadata =
        DelegationMetadata::try_from_bytes_with_discriminator(&delegation_metadata_data)?;

    // If the commit slot is greater or equal than the last update slot, we can proceed.
    // If the slot is less, we simply do not commit.
    // Since commit instructions are typically bundled, we return without error
    // so that correct commits are executed.
    if args.commit_record_slot < delegation_metadata.last_update_external_slot {
        msg!(
            "Slot {} is outdated, previous slot is {}. Skipping commit",
            args.commit_record_slot,
            delegation_metadata.last_update_external_slot
        );
        return Ok(());
    }

    // Once the account is marked as undelegatable, any subsequent commit should fail
    if delegation_metadata.is_undelegatable {
        msg!(
            "delegation metadata ({}) is already undelegated",
            args.delegation_metadata_account.key
        );
        return Err(DlpError::AlreadyUndelegated.into());
    }

    // Update delegation metadata undelegation flag
    delegation_metadata.is_undelegatable = args.allow_undelegation;
    delegation_metadata.to_bytes_with_discriminator(&mut delegation_metadata_data.as_mut())?;

    // Load delegation record
    let delegation_record_data = args.delegation_record_account.try_borrow_data()?;
    let delegation_record =
        DelegationRecord::try_from_bytes_with_discriminator(&delegation_record_data)?;

    // If there was an issue with the lamport accounting in the past, abort (this should never happen)
    if args.delegated_account.lamports() < delegation_record.lamports {
        msg!(
            "delegated account ({}) has less lamports than the delegation record indicates",
            args.delegated_account.key
        );
        return Err(DlpError::InvalidDelegatedState.into());
    }

    // If committed lamports are more than the previous lamports balance, deposit the difference in the commitment account
    // If committed lamports are less than the previous lamports balance, we have collateral to settle the balance at state finalization
    // We need to do that so that the finalizer already have all the lamports from the validators ready at finalize time
    // The finalizer can return any extra lamport to the validator during finalize, but this acts as the validator's proof of collateral
    if args.commit_record_lamports > delegation_record.lamports {
        let extra_lamports = args
            .commit_record_lamports
            .checked_sub(delegation_record.lamports)
            .ok_or(DlpError::Overflow)?;
        invoke(
            &transfer(
                args.validator.key,
                args.commit_state_account.key,
                extra_lamports,
            ),
            &[
                args.validator.clone(),
                args.commit_state_account.clone(),
                args.system_program.clone(),
            ],
        )?;
    }

    // Load the program configuration and validate it, if any
    let has_program_config =
        load_program_config(args.program_config_account, delegation_record.owner, false)?;
    if has_program_config {
        let program_config_data = args.program_config_account.try_borrow_data()?;
        let program_config =
            ProgramConfig::try_from_bytes_with_discriminator(&program_config_data)?;
        if !program_config
            .approved_validators
            .contains(args.validator.key)
        {
            msg!(
                "validator ({}) is not whitelisted in the program config",
                args.validator.key
            );
            return Err(DlpError::InvalidWhitelistProgramConfig.into());
        }
    }

    // Load the uninitialized PDAs
    let commit_state_bump = load_uninitialized_pda(
        args.commit_state_account,
        commit_state_seeds_from_delegated_account!(args.delegated_account.key),
        &crate::id(),
        true,
        "commit state account",
    )?;
    let commit_record_bump = load_uninitialized_pda(
        args.commit_record_account,
        commit_record_seeds_from_delegated_account!(args.delegated_account.key),
        &crate::id(),
        true,
        "commit record",
    )?;

    // Initialize the PDA containing the new committed state
    create_pda(
        args.commit_state_account,
        &crate::id(),
        args.commit_state_bytes.len(),
        commit_state_seeds_from_delegated_account!(args.delegated_account.key),
        commit_state_bump,
        args.system_program,
        args.validator,
    )?;

    // Initialize the PDA containing the record of the committed state
    create_pda(
        args.commit_record_account,
        &crate::id(),
        CommitRecord::size_with_discriminator(),
        commit_record_seeds_from_delegated_account!(args.delegated_account.key),
        commit_record_bump,
        args.system_program,
        args.validator,
    )?;

    // Initialize the commit record
    let commit_record = CommitRecord {
        identity: *args.validator.key,
        account: *args.delegated_account.key,
        slot: args.commit_record_slot,
        lamports: args.commit_record_lamports,
    };
    let mut commit_record_data = args.commit_record_account.try_borrow_mut_data()?;
    commit_record.to_bytes_with_discriminator(&mut commit_record_data)?;

    // Copy the new state to the initialized PDA
    let mut commit_state_data = args.commit_state_account.try_borrow_mut_data()?;
    (*commit_state_data).copy_from_slice(args.commit_state_bytes);

    // TODO - Add additional validation for the commitment, e.g. sufficient validator stake

    Ok(())
}
