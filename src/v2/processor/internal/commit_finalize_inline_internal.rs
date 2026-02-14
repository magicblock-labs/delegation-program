use pinocchio::{error::ProgramError, AccountView, ProgramResult};

use crate::{
    error::DlpError,
    pod_view::PodView,
    v2::{CommitFinalizeArgs, DelegationStateHeader, DelegationStateMut},
    v2_require, v2_require_eq, v2_require_eq_keys, v2_require_ge,
    v2_require_signer,
};

pub const DELEGATION_STATE_INLINE_SIZE: usize =
    4 + DelegationStateHeader::SPACE;

#[inline(always)]
pub fn process_commit_finalize_inline_internal<const DATA_IS_DIFF: bool>(
    validator: &AccountView,
    delegated_account: &AccountView,
    args: &CommitFinalizeArgs,
    state_or_diff: &[u8],
) -> ProgramResult {
    v2_require_signer!(validator);

    let delegated_account_lamports = delegated_account.lamports();

    let data = unsafe { delegated_account.borrow_unchecked_mut() };

    let (state_size, data) = unsafe { data.split_at_mut_unchecked(4) };

    let state_size = unsafe { *(state_size.as_ptr() as *const u32) };

    let (state_data, delegated_account_data) =
        unsafe { data.split_at_mut_unchecked(state_size as usize) };

    let (header, _) = unsafe {
        state_data.split_at_mut_unchecked(DelegationStateHeader::SPACE)
    };

    let mut state_view = DelegationStateMut::from_bytes(header)?;

    if DATA_IS_DIFF {
        Ok(())
    } else {
        if true {
            v2_require_eq_keys!(
                &state_view.bindings.validator_as_authority,
                validator.address(),
                DlpError::InvalidAuthority
            );

            v2_require_ge!(
                delegated_account_lamports,
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
        } else {
            v2_require!(
                // address_eq(
                //     &state_view.bindings.validator_as_authority,
                //     validator.address()
                // )
                // &&
                delegated_account_lamports >= state_view.original_lamports
                    && args.commit_id == state_view.last_commit_id + 1
                    && state_view.is_undelegatable.is_false(),
                DlpError::InvalidAuthority
            );
        }

        state_view.last_commit_id = args.commit_id;
        state_view.is_undelegatable = args.allow_undelegation.into();

        if false {
            delegated_account_data.copy_from_slice(state_or_diff);
        } else {
            #[cfg(any(target_os = "solana", target_arch = "bpf"))]
            unsafe {
                //pinocchio::syscalls::sol_memcpy_(
                solana_define_syscall::definitions::sol_memcpy_(
                    delegated_account_data.as_mut_ptr(),
                    state_or_diff.as_ptr(),
                    state_or_diff.len() as u64,
                );
            }

            #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
            unsafe {
                if false {
                    let rem = state_or_diff.len() % 8;
                    let len = state_or_diff.len() / 8;
                    core::ptr::copy_nonoverlapping(
                        state_or_diff.as_ptr() as *const u64,
                        delegated_account_data.as_mut_ptr() as *mut u64,
                        len,
                    );
                    if rem != 0 {
                        core::ptr::copy_nonoverlapping(
                            state_or_diff.as_ptr().add(len * 8),
                            delegated_account_data.as_mut_ptr().add(len * 8),
                            rem,
                        );
                    }
                } else {
                    core::ptr::copy_nonoverlapping(
                        state_or_diff.as_ptr(),
                        delegated_account_data.as_mut_ptr(),
                        state_or_diff.len(),
                    );
                }
            }
        }

        Ok(())
    }
}
