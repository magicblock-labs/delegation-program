use std::ops::Deref;

use pinocchio::{error::ProgramError, AccountView, Address, ProgramResult};

use crate::{
    error::DlpError,
    pod_view::PodView,
    v2::{
        processor::internal::process_commit_finalize_inline_internal,
        CommitFinalizeArgsWithBuffer, DelegationStateHeader,
        DelegationStateMut,
    },
    v2_require, v2_require_eq, v2_require_eq_keys, v2_require_ge,
    v2_require_signer,
};

#[inline(always)]
pub fn process_commit_finalize_inline(
    accounts: &[AccountView],
    //data: &[u8],
    ixdata: *const u8,
    ixdatalen: usize,
) -> ProgramResult {
    // let [
    //     validator, // force multi-line
    //     delegated_account,
    // ] = crate::v2_require_n_accounts!(accounts, 2);

    let validator = unsafe { accounts.get_unchecked(0) };
    let delegated_account = unsafe { accounts.get_unchecked(1) };

    //let args = CommitFinalizeArgsWithBuffer::from_bytes(data)?;
    let args = CommitFinalizeArgsWithBuffer::from_bytes_ptr(ixdata, ixdatalen)?;

    if args.data_is_diff.is_true() {
        process_commit_finalize_inline_internal::<true>(
            validator,
            delegated_account,
            args.deref(),
            args.buffer,
        )
    } else {
        process_commit_finalize_inline_internal::<false>(
            validator,
            delegated_account,
            args.deref(),
            args.buffer,
        )
    }
}

/// Commit a new state, or a diff, directly to the delegated account. Unlike, CommitState and
/// CommitDiff variants, this instruction does not write to any temporary account first. In other
/// words, this instruction commits and finalizes both.
///
/// Accounts:
///
/// 0: `[signer]`   the validator requesting the commit
/// 1: `[]`         the delegated account
/// 2: `[]`         the delegation record
/// 3: `[writable]` the delegation metadata
/// 4: `[]`         the validator fees vault
/// 5: `[]`         the program config account
/// 6: `[]`         system program
///
/// Instruction Data: CommitFinalizeArgsWithBuffer
///

const VALIDATOR: Address =
    pinocchio::address::address!("tEsT3eV6RFCWs1BZ7AXTzasHqTtMnMLCB2tjQ42TDXD");

#[allow(dead_code)]
//#[inline(always)]
pub fn process_commit_finalize_inline_101_cu(
    accounts: &[AccountView],
    //data: &[u8],
    ixdata: *const u8,
    ixdatalen: usize,
) -> ProgramResult {
    //let [
    //    validator, // force multi-line
    //    delegated_account,
    //] = v2_require_n_accounts!(accounts, 2);

    let validator = unsafe { accounts.get_unchecked(0) };
    let delegated_account = unsafe { accounts.get_unchecked(1) };

    //let args = CommitFinalizeArgsWithBuffer::from_bytes(data)?;
    let args = CommitFinalizeArgsWithBuffer::from_bytes_ptr(ixdata, ixdatalen)?;

    v2_require_signer!(validator);

    v2_require!(
        args.data_is_diff.is_false(),
        ProgramError::InvalidInstructionData
    );

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

    if true {
        if false {
            v2_require_eq_keys!(
                &state_view.bindings.validator_as_authority,
                validator.address(),
                DlpError::InvalidAuthority
            );
        } else {
            v2_require_eq_keys!(
                &VALIDATOR,
                validator.address(),
                DlpError::InvalidAuthority
            );
        }

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
        delegated_account_data.copy_from_slice(args.buffer);
    } else {
        #[cfg(any(target_os = "solana", target_arch = "bpf"))]
        unsafe {
            //pinocchio::syscalls::sol_memcpy_(
            solana_define_syscall::definitions::sol_memcpy_(
                delegated_account_data.as_mut_ptr(),
                args.buffer.as_ptr(),
                args.buffer.len() as u64,
            );
        }

        #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
        unsafe {
            if false {
                let rem = args.buffer.len() % 8;
                let len = args.buffer.len() / 8;
                core::ptr::copy_nonoverlapping(
                    args.buffer.as_ptr() as *const u64,
                    delegated_account_data.as_mut_ptr() as *mut u64,
                    len,
                );
                if rem != 0 {
                    core::ptr::copy_nonoverlapping(
                        args.buffer.as_ptr().add(len * 8),
                        delegated_account_data.as_mut_ptr().add(len * 8),
                        rem,
                    );
                }
            } else {
                core::ptr::copy_nonoverlapping(
                    args.buffer.as_ptr(),
                    delegated_account_data.as_mut_ptr(),
                    args.buffer.len(),
                );
            }
        }
    }

    Ok(())
}
