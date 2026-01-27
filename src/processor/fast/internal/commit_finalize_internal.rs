use pinocchio::{
    address::{address_eq, PDA_MARKER},
    error::ProgramError,
    AccountView, Address,
};
use pinocchio_log::log;

use crate::{
    apply_diff_in_place,
    args::CommitBumps,
    error::DlpError,
    pda,
    pod_view::PodView,
    processor::fast::NewState,
    require, require_eq, require_eq_keys, require_ge,
    require_initialized_pda_fast, require_owned_by, require_signer,
    state::{DelegationMetadataFast, DelegationRecord},
};

/// Arguments for the commit state internal function
pub(crate) struct CommitFinalizeInternalArgs<'a> {
    pub(crate) bumps: &'a CommitBumps,
    pub(crate) new_state: NewState<'a>,
    pub(crate) commit_id: u64,
    pub(crate) allow_undelegation: bool,
    pub(crate) validator: &'a AccountView,
    pub(crate) delegated_account: &'a AccountView,
    pub(crate) delegation_record_account: &'a AccountView,
    pub(crate) delegation_metadata_account: &'a AccountView,
    pub(crate) validator_fees_vault: &'a AccountView,
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
            args.delegated_account.address().as_ref(),
            &[args.bumps.delegation_record],
            crate::fast::ID.as_ref(),
            PDA_MARKER
        ],
        false
    );

    require_initialized_pda_fast!(
        args.delegation_metadata_account,
        &[
            pda::DELEGATION_METADATA_TAG,
            args.delegated_account.address().as_ref(),
            &[args.bumps.delegation_metadata],
            crate::fast::ID.as_ref(),
            PDA_MARKER
        ],
        true
    );

    require_initialized_pda_fast!(
        args.validator_fees_vault,
        &[
            pda::VALIDATOR_FEES_VAULT_TAG,
            args.validator.address().as_ref(),
            &[args.bumps.validator_fees_vault],
            crate::fast::ID.as_ref(),
            PDA_MARKER
        ],
        false
    );

    // validate and update metadata
    {
        let mut metadata = DelegationMetadataFast::from_account(
            args.delegation_metadata_account,
        )?;

        let prev_id = metadata.replace_last_update_nonce(args.commit_id);

        require_eq!(args.commit_id, prev_id + 1, DlpError::NonceOutOfOrder);

        require!(
            !metadata.replace_is_undelegatable(args.allow_undelegation),
            DlpError::AlreadyUndelegated
        );
    }

    let delegation_record_data = args.delegation_record_account.try_borrow()?;
    let delegation_record =
        DelegationRecord::try_view_from(&delegation_record_data.as_ref()[8..])?;

    // Check that the authority is allowed to commit
    require_eq_keys!(
        &Address::from(delegation_record.authority.to_bytes()),
        args.validator.address(),
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
    let mut delegated_account_data = args.delegated_account.try_borrow_mut()?;
    match args.new_state {
        NewState::FullBytes(bytes) => {
            (*delegated_account_data).copy_from_slice(bytes)
        }
        NewState::Diff(diff) => {
            apply_diff_in_place(&mut delegated_account_data, &diff)?;
        }
    }

    Ok(())
}
