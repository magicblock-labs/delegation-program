use borsh::BorshDeserialize;
use pinocchio::{
    address::address_eq, cpi::Signer, error::ProgramError, instruction::seeds,
    AccountView, Address, ProgramResult,
};
use pinocchio_log::log;
use pinocchio_system::instructions as system;

use super::to_pinocchio_program_error;
use crate::{
    args::CommitStateArgs,
    error::DlpError,
    merge_diff_copy, pda,
    processor::fast::utils::{
        pda::create_pda,
        requires::{
            require_initialized_delegation_metadata,
            require_initialized_delegation_record,
            require_initialized_validator_fees_vault, require_owned_pda,
            require_program_config, require_signer, require_uninitialized_pda,
            CommitRecordCtx, CommitStateAccountCtx,
        },
    },
    state::{
        CommitRecord, DelegationMetadata, DelegationRecord, ProgramConfig,
    },
    DiffSet,
};

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
    accounts: &[AccountView],
    data: &[u8],
) -> ProgramResult {
    let args = CommitStateArgs::try_from_slice(data)
        .map_err(|_| ProgramError::BorshIoError)?;

    let commit_record_lamports = args.lamports;
    let commit_record_nonce = args.nonce;
    let allow_undelegation = args.allow_undelegation;

    let [validator, delegated_account, commit_state_account, commit_record_account, delegation_record_account, delegation_metadata_account, validator_fees_vault, program_config_account, _system_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    let commit_args = CommitStateInternalArgs {
        commit_state_bytes: NewState::FullBytes(&args.data),
        commit_record_lamports,
        commit_record_nonce,
        allow_undelegation,
        validator,
        delegated_account,
        commit_state_account,
        commit_record_account,
        delegation_record_account,
        delegation_metadata_account,
        validator_fees_vault,
        program_config_account,
    };

    process_commit_state_internal(commit_args)
}

pub(crate) enum NewState<'a> {
    FullBytes(&'a [u8]),
    Diff(DiffSet<'a>),
}

impl NewState<'_> {
    pub fn data_len(&self) -> usize {
        match self {
            NewState::FullBytes(bytes) => bytes.len(),
            NewState::Diff(diff) => diff.changed_len(),
        }
    }
}

/// Arguments for the commit state internal function
pub(crate) struct CommitStateInternalArgs<'a> {
    pub(crate) commit_state_bytes: NewState<'a>,
    pub(crate) commit_record_lamports: u64,
    pub(crate) commit_record_nonce: u64,
    pub(crate) allow_undelegation: bool,
    pub(crate) validator: &'a AccountView,
    pub(crate) delegated_account: &'a AccountView,
    pub(crate) commit_state_account: &'a AccountView,
    pub(crate) commit_record_account: &'a AccountView,
    pub(crate) delegation_record_account: &'a AccountView,
    pub(crate) delegation_metadata_account: &'a AccountView,
    pub(crate) validator_fees_vault: &'a AccountView,
    pub(crate) program_config_account: &'a AccountView,
}

/// Commit a new state of a delegated Pda
pub(crate) fn process_commit_state_internal(
    args: CommitStateInternalArgs,
) -> Result<(), ProgramError> {
    // Check that the origin account is delegated
    require_owned_pda(
        args.delegated_account,
        &crate::fast::ID,
        "delegated account",
    )?;
    require_signer(args.validator, "validator account")?;
    require_initialized_delegation_record(
        args.delegated_account,
        args.delegation_record_account,
        false,
    )?;
    require_initialized_delegation_metadata(
        args.delegated_account,
        args.delegation_metadata_account,
        true,
    )?;
    require_initialized_validator_fees_vault(
        args.validator,
        args.validator_fees_vault,
        false,
    )?;

    // Read delegation metadata
    let mut delegation_metadata_data =
        args.delegation_metadata_account.try_borrow_mut()?;
    let mut delegation_metadata =
        DelegationMetadata::try_from_bytes_with_discriminator(
            &delegation_metadata_data,
        )
        .map_err(to_pinocchio_program_error)?;

    // To preserve correct history of account updates we require sequential commits
    if args.commit_record_nonce != delegation_metadata.last_update_nonce + 1 {
        log!(
            "Nonce {} is incorrect, previous nonce is {}. Rejecting commit",
            args.commit_record_nonce,
            delegation_metadata.last_update_nonce
        );
        return Err(DlpError::NonceOutOfOrder.into());
    }

    // Once the account is marked as undelegatable, any subsequent commit should fail
    if delegation_metadata.is_undelegatable {
        log!("delegation metadata is already undelegated: ");
        args.delegation_metadata_account.address().log();
        return Err(DlpError::AlreadyUndelegated.into());
    }

    // Update delegation metadata undelegation flag
    delegation_metadata.is_undelegatable = args.allow_undelegation;
    delegation_metadata
        .to_bytes_with_discriminator(&mut delegation_metadata_data.as_mut())
        .map_err(to_pinocchio_program_error)?;

    // Load delegation record
    let delegation_record_data = args.delegation_record_account.try_borrow()?;
    let delegation_record =
        DelegationRecord::try_from_bytes_with_discriminator(
            &delegation_record_data,
        )
        .map_err(to_pinocchio_program_error)?;

    // Check that the authority is allowed to commit
    if !address_eq(
        &delegation_record.authority.to_bytes().into(),
        args.validator.address(),
    ) {
        log!("validator is not the delegation authority. validator: ");
        args.validator.address().log();
        log!("delegation authority: ");
        Address::from(delegation_record.authority.to_bytes()).log();
        return Err(DlpError::InvalidAuthority.into());
    }

    // If there was an issue with the lamport accounting in the past, abort (this should never happen)
    if args.delegated_account.lamports() < delegation_record.lamports {
        log!(
            "delegated account has less lamports than the delegation record indicates. delegation account: ");
        args.delegated_account.address().log();
        return Err(DlpError::InvalidDelegatedState.into());
    }

    // If committed lamports are more than the previous lamports balance, deposit the difference in the commitment account
    // If committed lamports are less than the previous lamports balance, we have collateral to settle the balance at state finalization
    // We need to do that so that the finalizer already have all the lamports from the validators ready at finalize time
    // The finalizer can return any extra lamport to the validator during finalize, but this acts as the validator's proof of collateral
    if args.commit_record_lamports > delegation_record.lamports {
        system::Transfer {
            from: args.validator,
            to: args.commit_state_account,
            lamports: args.commit_record_lamports - delegation_record.lamports,
        }
        .invoke()?;
    }

    // Load the program configuration and validate it, if any
    let has_program_config = require_program_config(
        args.program_config_account,
        &Address::from(delegation_record.owner.to_bytes()),
        false,
    )?;
    if has_program_config {
        let program_config_data = args.program_config_account.try_borrow()?;

        let program_config = ProgramConfig::try_from_bytes_with_discriminator(
            &program_config_data,
        )
        .map_err(to_pinocchio_program_error)?;
        if !program_config
            .approved_validators
            .contains(&args.validator.address().to_bytes().into())
        {
            log!("validator is not whitelisted in the program config: ");
            args.validator.address().log();
            return Err(DlpError::InvalidWhitelistProgramConfig.into());
        }
    }

    // Load the uninitialized PDAs
    let commit_state_bump = require_uninitialized_pda(
        args.commit_state_account,
        &[
            pda::COMMIT_STATE_TAG,
            args.delegated_account.address().as_ref(),
        ],
        &crate::fast::ID,
        true,
        CommitStateAccountCtx,
    )?;
    let commit_record_bump = require_uninitialized_pda(
        args.commit_record_account,
        &[
            pda::COMMIT_RECORD_TAG,
            args.delegated_account.address().as_ref(),
        ],
        &crate::fast::ID,
        true,
        CommitRecordCtx,
    )?;

    // Initialize the PDA containing the new committed state
    create_pda(
        args.commit_state_account,
        &crate::fast::ID,
        args.commit_state_bytes.data_len(),
        &[Signer::from(&seeds!(
            pda::COMMIT_STATE_TAG,
            args.delegated_account.address().as_ref(),
            &[commit_state_bump]
        ))],
        args.validator,
    )?;

    // Initialize the PDA containing the record of the committed state
    create_pda(
        args.commit_record_account,
        &crate::fast::ID,
        CommitRecord::size_with_discriminator(),
        &[Signer::from(&seeds!(
            pda::COMMIT_RECORD_TAG,
            args.delegated_account.address().as_ref(),
            &[commit_record_bump]
        ))],
        args.validator,
    )?;

    // Initialize the commit record
    let commit_record = CommitRecord {
        identity: args.validator.address().to_bytes().into(),
        account: args.delegated_account.address().to_bytes().into(),
        nonce: args.commit_record_nonce,
        lamports: args.commit_record_lamports,
    };
    let mut commit_record_data = args.commit_record_account.try_borrow_mut()?;
    commit_record
        .to_bytes_with_discriminator(&mut commit_record_data)
        .map_err(to_pinocchio_program_error)?;

    // Copy the new state to the initialized PDA
    let mut commit_state_data = args.commit_state_account.try_borrow_mut()?;

    match args.commit_state_bytes {
        NewState::FullBytes(bytes) => {
            (*commit_state_data).copy_from_slice(bytes)
        }
        NewState::Diff(diff) => {
            let original_data = args.delegated_account.try_borrow()?;
            merge_diff_copy(&mut commit_state_data, &original_data, &diff)?;
        }
    }

    // TODO - Add additional validation for the commitment, e.g. sufficient validator stake

    Ok(())
}
