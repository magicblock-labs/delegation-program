use borsh::BorshDeserialize;
use pinocchio::instruction::Signer;
use pinocchio::pubkey::{self, pubkey_eq};
use pinocchio::seeds;
use pinocchio::{
    account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey, ProgramResult,
};
use pinocchio_log::log;
use pinocchio_system::instructions as system;

use crate::args::CommitStateArgs;
use crate::error::DlpError;
use crate::pda;
use crate::processor::fast::utils::{
    pda::create_pda,
    requires::{
        require_initialized_delegation_metadata, require_initialized_delegation_record,
        require_initialized_validator_fees_vault, require_owned_pda, require_program_config,
        require_signer, require_uninitialized_pda,
    },
};
use crate::state::{CommitRecord, DelegationMetadata, DelegationRecord, ProgramConfig};

use super::to_pinocchio_program_error;

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
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let args = CommitStateArgs::try_from_slice(data).map_err(|_| ProgramError::BorshIoError)?;

    let commit_state_bytes: &[u8] = args.data.as_ref();
    let commit_record_lamports = args.lamports;
    let commit_record_nonce = args.nonce;
    let allow_undelegation = args.allow_undelegation;

    let [validator, delegated_account, commit_state_account, commit_record_account, delegation_record_account, delegation_metadata_account, validator_fees_vault, program_config_account, _system_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    let commit_args = CommitStateInternalArgs {
        commit_state_bytes,
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

/// Arguments for the commit state internal function
pub(crate) struct CommitStateInternalArgs<'a> {
    pub(crate) commit_state_bytes: &'a [u8],
    pub(crate) commit_record_lamports: u64,
    pub(crate) commit_record_nonce: u64,
    pub(crate) allow_undelegation: bool,
    pub(crate) validator: &'a AccountInfo,
    pub(crate) delegated_account: &'a AccountInfo,
    pub(crate) commit_state_account: &'a AccountInfo,
    pub(crate) commit_record_account: &'a AccountInfo,
    pub(crate) delegation_record_account: &'a AccountInfo,
    pub(crate) delegation_metadata_account: &'a AccountInfo,
    pub(crate) validator_fees_vault: &'a AccountInfo,
    pub(crate) program_config_account: &'a AccountInfo,
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
    require_initialized_validator_fees_vault(args.validator, args.validator_fees_vault, false)?;

    // Read delegation metadata
    let mut delegation_metadata_data = args.delegation_metadata_account.try_borrow_mut_data()?;
    let mut delegation_metadata =
        DelegationMetadata::try_from_bytes_with_discriminator(&delegation_metadata_data)
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
        pubkey::log(args.delegation_metadata_account.key());
        return Err(DlpError::AlreadyUndelegated.into());
    }

    // Update delegation metadata undelegation flag
    delegation_metadata.is_undelegatable = args.allow_undelegation;
    delegation_metadata
        .to_bytes_with_discriminator(&mut delegation_metadata_data.as_mut())
        .map_err(to_pinocchio_program_error)?;

    // Load delegation record
    let delegation_record_data = args.delegation_record_account.try_borrow_data()?;
    let delegation_record =
        DelegationRecord::try_from_bytes_with_discriminator(&delegation_record_data)
            .map_err(to_pinocchio_program_error)?;

    // Check that the authority is allowed to commit
    if !pubkey_eq(delegation_record.authority.as_array(), args.validator.key())
        && !pubkey_eq(delegation_record.authority.as_array(), &Pubkey::default())
    {
        log!("validator is not the delegation authority. validator: ");
        pubkey::log(args.validator.key());
        log!("delegation authority: ");
        pubkey::log(delegation_record.authority.as_array());
        return Err(DlpError::InvalidAuthority.into());
    }

    // If there was an issue with the lamport accounting in the past, abort (this should never happen)
    if args.delegated_account.lamports() < delegation_record.lamports {
        log!(
            "delegated account has less lamports than the delegation record indicates. delegation account: ");
        pubkey::log(args.delegated_account.key());
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

        system::Transfer {
            from: args.validator,
            to: args.commit_state_account,
            lamports: extra_lamports,
        }
        .invoke()?;
    }

    // Load the program configuration and validate it, if any
    let has_program_config = require_program_config(
        args.program_config_account,
        delegation_record.owner.as_array(),
        false,
    )?;
    if has_program_config {
        let program_config_data = args.program_config_account.try_borrow_data()?;

        let program_config = ProgramConfig::try_from_bytes_with_discriminator(&program_config_data)
            .map_err(to_pinocchio_program_error)?;
        if !program_config
            .approved_validators
            .contains(&(*args.validator.key()).into())
        {
            log!("validator is not whitelisted in the program config: ");
            pubkey::log(args.validator.key());
            return Err(DlpError::InvalidWhitelistProgramConfig.into());
        }
    }

    // Load the uninitialized PDAs
    let commit_state_bump = require_uninitialized_pda(
        args.commit_state_account,
        &[pda::COMMIT_STATE_TAG, args.delegated_account.key()],
        &crate::fast::ID,
        true,
        "commit state account",
    )?;
    let commit_record_bump = require_uninitialized_pda(
        args.commit_record_account,
        &[pda::COMMIT_RECORD_TAG, args.delegated_account.key()],
        &crate::fast::ID,
        true,
        "commit record",
    )?;

    // Initialize the PDA containing the new committed state
    create_pda(
        args.commit_state_account,
        &crate::fast::ID,
        args.commit_state_bytes.len(),
        &[Signer::from(&seeds!(
            pda::COMMIT_STATE_TAG,
            args.delegated_account.key(),
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
            args.delegated_account.key(),
            &[commit_record_bump]
        ))],
        args.validator,
    )?;

    // Initialize the commit record
    let commit_record = CommitRecord {
        identity: (*args.validator.key()).into(),
        account: (*args.delegated_account.key()).into(),
        nonce: args.commit_record_nonce,
        lamports: args.commit_record_lamports,
    };
    let mut commit_record_data = args.commit_record_account.try_borrow_mut_data()?;
    commit_record
        .to_bytes_with_discriminator(&mut commit_record_data)
        .map_err(to_pinocchio_program_error)?;

    // Copy the new state to the initialized PDA
    let mut commit_state_data = args.commit_state_account.try_borrow_mut_data()?;
    (*commit_state_data).copy_from_slice(args.commit_state_bytes);

    // TODO - Add additional validation for the commitment, e.g. sufficient validator stake

    Ok(())
}
