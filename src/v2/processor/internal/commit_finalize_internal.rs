use pinocchio::{error::ProgramError, AccountView};

use crate::{
    apply_diff_in_place, error::DlpError, pod_view::PodView,
    processor::fast::NewState, v2::DelegationStateHeader, v2_require,
    v2_require_eq, v2_require_eq_keys, v2_require_eq_keys_unsafe,
    v2_require_ge, v2_require_owned_by, v2_require_signer,
};

/// Arguments for the commit state internal function
pub(crate) struct CommitFinalizeInternalArgs<'a> {
    pub(crate) new_state: NewState<'a>,
    pub(crate) commit_id: u64,
    pub(crate) allow_undelegation: bool,
    pub(crate) validator: &'a AccountView,
    pub(crate) delegated_account: &'a AccountView,
    pub(crate) delegation_state: &'a AccountView,
}

#[inline(always)]
pub(crate) fn process_commit_finalize_internal(
    args: CommitFinalizeInternalArgs,
) -> Result<(), ProgramError> {
    // check delegated_account is actually delegated to the DLP
    if true {
        v2_require_signer!(args.validator);

        //// we do not really need to check this as the write will fail anyway
        // v2_require_owned_by!(args.delegated_account, &crate::fast::ID);

        //DelegationStateHeader::validate(args.delegation_state)?;

        if true {
            let pda = solana_sha256_hasher::hashv(&[
                DelegationStateHeader::SEED,
                args.delegated_account.address().as_ref(),
                &[255], // TODO: replace me with args.bump
                crate::fast::ID.as_ref(),
                solana_address::PDA_MARKER,
            ])
            .to_bytes()
            .into();

            v2_require_eq_keys!(
                &pda,
                args.delegation_state.address(),
                ProgramError::Immutable
            );
        }

        v2_require_owned_by!(args.delegation_state, &crate::fast::ID);

        #[cfg(feature = "unsafe")]
        let state_data =
            unsafe { args.delegation_state.borrow_unchecked_mut() };

        #[cfg(not(feature = "unsafe"))]
        let mut state_data = args.delegation_state.try_borrow_mut()?;

        v2_require_ge!(
            state_data.len(),
            DelegationStateHeader::SPACE,
            ProgramError::InvalidAccountData
        );

        let (header, _) = state_data.split_at_mut(DelegationStateHeader::SPACE);

        let state_view = unsafe {
            &mut *(header.as_mut_ptr() as *mut DelegationStateHeader)
        };

        v2_require_eq!(
            //unsafe { (header.as_ptr() as *const u64).read() },
            u64::from_le_bytes(state_view.discriminator),
            DelegationStateHeader::DISCRIMINATOR_FAST,
            ProgramError::InvalidAccountData
        );

        // If there was an issue with the lamport accounting in the past, abort (this should never happen)
        v2_require_ge!(
            args.delegated_account.lamports(),
            state_view.original_lamports,
            DlpError::InvalidDelegatedState
        );

        v2_require_eq!(
            args.commit_id,
            state_view.last_commit_id + 1,
            DlpError::NonceOutOfOrder
        );

        v2_require!(
            state_view.is_undelegatable.is_false(),
            DlpError::NonceOutOfOrder
        );

        state_view.last_commit_id = args.commit_id;
        state_view.is_undelegatable = args.allow_undelegation.into();

        if true {
            // key comparision cost: 12 CU
            v2_require_eq_keys!(
                &state_view.bindings.delegated_account,
                args.delegated_account.address(),
                DlpError::InvalidAuthority
            );

            v2_require_eq_keys!(
                &state_view.bindings.validator_as_authority,
                args.validator.address(),
                DlpError::InvalidAuthority
            );

            // TODO (snawaz): why do we need validator_fees_vault here?
            // v2_require_eq_keys!(
            //     &state_view.bindings.validator_fees_vault,
            //     args.validator_fees_vault.address(),
            //     DlpError::InvalidAuthority
            // );
        } else if false {
            // key comparision cost: 12 CU
            unsafe {
                v2_require_eq_keys_unsafe!(
                    &state_view.bindings.delegated_account,
                    args.delegated_account.address(),
                    DlpError::InvalidAuthority
                );

                v2_require_eq_keys_unsafe!(
                    &state_view.bindings.validator_as_authority,
                    args.validator.address(),
                    DlpError::InvalidAuthority
                );
            }
        }
    }

    if args.delegated_account.data_len() != args.new_state.data_len() {
        args.delegated_account.resize(args.new_state.data_len())?;
    }

    // copy the new state to the delegated account
    #[cfg(feature = "unsafe")]
    let mut delegated_account_data =
        unsafe { args.delegated_account.borrow_unchecked_mut() };

    #[cfg(not(feature = "unsafe"))]
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
