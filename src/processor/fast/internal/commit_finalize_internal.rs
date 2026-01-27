use pinocchio::pubkey::{self, pubkey_eq, PDA_MARKER};
use pinocchio::{account_info::AccountInfo, program_error::ProgramError};
use pinocchio_log::log;

use crate::args::CommitBumps;
use crate::error::DlpError;
use crate::pod_view::PodView;
use crate::processor::fast::NewState;
use crate::state::{DelegationMetadataFast, DelegationRecord};
use crate::{
    apply_diff_in_place, pda, require, require_eq, require_eq_keys, require_ge,
    require_initialized_pda_fast, require_owned_by, require_signer,
};

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
}

/// Commit a new state of a delegated Pda
pub(crate) fn process_commit_finalize_internal(
    args: CommitFinalizeInternalArgs,
) -> Result<(), ProgramError> {
    // check delegated_account is actually delegated to the DLP
    require_owned_by!(args.delegated_account, &crate::fast::ID);

    require_signer!(args.validator);

    require_initialized_pda_fast!(
        args.delegation_record_account,
        &[
            pda::DELEGATION_RECORD_TAG,
            args.delegated_account.key(),
            &[args.bumps.delegation_record],
            &crate::fast::ID,
            PDA_MARKER
        ],
        false
    );

    require_initialized_pda_fast!(
        args.delegation_metadata_account,
        &[
            pda::DELEGATION_METADATA_TAG,
            args.delegated_account.key(),
            &[args.bumps.delegation_metadata],
            &crate::fast::ID,
            PDA_MARKER
        ],
        true
    );

    require_initialized_pda_fast!(
        args.validator_fees_vault,
        &[
            pda::VALIDATOR_FEES_VAULT_TAG,
            args.validator.key(),
            &[args.bumps.validator_fees_vault],
            &crate::fast::ID,
            PDA_MARKER
        ],
        false
    );

    // validate and update metadata
    {
        let mut metadata = DelegationMetadataFast::from_account(args.delegation_metadata_account)?;

        let prev_id = metadata.replace_last_update_nonce(args.commit_id);

        require_eq!(args.commit_id, prev_id + 1, DlpError::NonceOutOfOrder);

        require!(
            !metadata.replace_is_undelegatable(args.allow_undelegation),
            DlpError::AlreadyUndelegated
        );
    }

    let delegation_record_data = args.delegation_record_account.try_borrow_data()?;
    let delegation_record = DelegationRecord::try_view_from(&delegation_record_data.as_ref()[8..])?;

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

    args.delegated_account.resize(args.new_state.data_len())?;

    // copy the new state to the delegated account
    let mut delegated_account_data = args.delegated_account.try_borrow_mut_data()?;
    match args.new_state {
        NewState::FullBytes(bytes) => (*delegated_account_data).copy_from_slice(bytes),
        NewState::Diff(diff) => {
            apply_diff_in_place(&mut delegated_account_data, &diff)?;
        }
    }

    Ok(())
}
