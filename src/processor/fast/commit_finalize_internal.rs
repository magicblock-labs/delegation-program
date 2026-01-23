use pinocchio::pubkey::{self, pubkey_eq, PDA_MARKER};
use pinocchio::{account_info::AccountInfo, program_error::ProgramError};
use pinocchio_log::log;

use crate::args::CommitBumps;
use crate::error::DlpError;
use crate::pod_view::PodView;
use crate::processor::fast::NewState;
use crate::state::{DelegationMetadata, DelegationMetadataFast, DelegationRecord, ProgramConfig};
use crate::{
    apply_diff_in_place, pda, require, require_eq, require_eq_keys, require_ge,
    require_initialized_pda, require_initialized_pda_unsafe, require_owned_by,
    require_program_config, require_program_config_unsafe, require_signer,
};

use super::to_pinocchio_program_error;

/// Arguments for the commit state internal function
pub(crate) struct CommitFinalizeInternalArgs<'a> {
    pub(crate) bumps: &'a CommitBumps,
    pub(crate) new_state: NewState<'a>,
    pub(crate) commit_id: u64,
    pub(crate) allow_undelegation: bool,
    pub(crate) validator: &'a AccountInfo,
    pub(crate) delegated_account: &'a AccountInfo,
    pub(crate) delegation_record_account: &'a AccountInfo,
    pub(crate) delegation_metadata_account: &'a AccountInfo,
    pub(crate) validator_fees_vault: &'a AccountInfo,
    pub(crate) program_config_account: &'a AccountInfo,
}

/// Commit a new state of a delegated Pda
pub(crate) fn process_commit_finalize_internal(
    args: CommitFinalizeInternalArgs,
) -> Result<(), ProgramError> {
    // check delegated_account is actually delegated to the DLP
    require_owned_by!(args.delegated_account, &crate::fast::ID);

    require_signer!(args.validator);

    const USE_SAFE: bool = false;

    if USE_SAFE {
        require_initialized_pda!(
            args.delegation_record_account,
            &[
                pda::DELEGATION_RECORD_TAG,
                args.delegated_account.key(),
                &[args.bumps.delegation_record]
            ],
            &crate::fast::ID,
            false
        );

        require_initialized_pda!(
            args.delegation_metadata_account,
            &[
                pda::DELEGATION_METADATA_TAG,
                args.delegated_account.key(),
                &[args.bumps.delegation_metadata]
            ],
            &crate::fast::ID,
            true
        );

        require_initialized_pda!(
            args.validator_fees_vault,
            &[
                pda::VALIDATOR_FEES_VAULT_TAG,
                args.validator.key(),
                &[args.bumps.validator_fees_vault]
            ],
            &crate::fast::ID,
            false
        );
    } else {
        require_initialized_pda_unsafe!(
            args.delegation_record_account,
            &[
                pda::DELEGATION_RECORD_TAG,
                args.delegated_account.key(),
                &[args.bumps.delegation_record],
                &crate::fast::ID,
                PDA_MARKER
            ],
            &crate::fast::ID,
            false
        );

        require_initialized_pda_unsafe!(
            args.delegation_metadata_account,
            &[
                pda::DELEGATION_METADATA_TAG,
                args.delegated_account.key(),
                &[args.bumps.delegation_metadata],
                &crate::fast::ID,
                PDA_MARKER
            ],
            &crate::fast::ID,
            true
        );

        require_initialized_pda_unsafe!(
            args.validator_fees_vault,
            &[
                pda::VALIDATOR_FEES_VAULT_TAG,
                args.validator.key(),
                &[args.bumps.validator_fees_vault],
                &crate::fast::ID,
                PDA_MARKER
            ],
            &crate::fast::ID,
            false
        );
    }

    if false {
        // Read delegation metadata
        let mut delegation_metadata_data =
            args.delegation_metadata_account.try_borrow_mut_data()?;
        let mut delegation_metadata =
            DelegationMetadata::try_from_bytes_with_discriminator(&delegation_metadata_data)
                .map_err(to_pinocchio_program_error)?;

        // To preserve correct history of account updates we require sequential commits
        if args.commit_id != delegation_metadata.last_update_nonce + 1 {
            log!(
                "Nonce {} is incorrect, previous nonce is {}. Rejecting commit",
                args.commit_id,
                delegation_metadata.last_update_nonce
            );
            return Err(DlpError::NonceOutOfOrder.into());
        }
        delegation_metadata.last_update_nonce += 1;

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
    } else {
        let mut metadata = DelegationMetadataFast::from_account(args.delegation_metadata_account)?;

        let prev_id = metadata.replace_last_update_nonce(args.commit_id);

        require_eq!(args.commit_id, prev_id + 1, DlpError::NonceOutOfOrder);

        require!(
            !metadata.replace_is_undelegatable(args.allow_undelegation),
            DlpError::AlreadyUndelegated
        );
    }
    // Load delegation record
    let delegation_record_data = args.delegation_record_account.try_borrow_data()?;
    let delegation_record = if false {
        DelegationRecord::try_from_bytes_with_discriminator(&delegation_record_data)
            .map_err(to_pinocchio_program_error)?
    } else {
        DelegationRecord::try_view_from(&delegation_record_data.as_ref()[8..])?
    };

    // Check that the authority is allowed to commit
    require_eq_keys!(
        delegation_record.authority.as_array(),
        args.validator.key(),
        DlpError::InvalidAuthority
    );

    // If there was an issue with the lamport accounting in the past, abort (this should never happen)
    require_ge!(
        args.delegated_account.lamports(),
        delegation_record.lamports,
        DlpError::InvalidDelegatedState
    );

    // if args.commit_record_lamports > delegation_record.lamports {
    //     system::Transfer {
    //         from: args.validator,
    //         to: args.commit_state_account,
    //         lamports: args.commit_record_lamports - delegation_record.lamports,
    //     }
    //     .invoke()?;
    // }

    // OPTIMIZE 1
    if false {
        // Load the program configuration and validate it, if any
        let has_program_config = if USE_SAFE {
            require_program_config!(
                args.program_config_account,
                delegation_record.owner.as_array(),
                args.bumps.program_config,
                false
            )
        } else {
            require_program_config_unsafe!(
                args.program_config_account,
                delegation_record.owner.as_array(),
                args.bumps.program_config,
                false
            )
        };
        if has_program_config {
            let program_config_data = args.program_config_account.try_borrow_data()?;

            let program_config =
                ProgramConfig::try_from_bytes_with_discriminator(&program_config_data)
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
    }

    args.delegated_account.resize(args.new_state.data_len())?;

    // Copy the new state to the initialized PDA
    let mut delegated_account_data = args.delegated_account.try_borrow_mut_data()?;
    match args.new_state {
        NewState::FullBytes(bytes) => (*delegated_account_data).copy_from_slice(bytes),
        NewState::Diff(diff) => {
            apply_diff_in_place(&mut delegated_account_data, &diff)?;
        }
    }

    Ok(())
}
