use pinocchio::{error::ProgramError, AccountView, ProgramResult};

use crate::{
    error::DlpError,
    pod_view::PodView,
    v2::{
        CommitFinalizeArgsWithBuffer, DelegationStateHeader, DelegationStateMut,
    },
    v2_require, v2_require_eq, v2_require_eq_keys, v2_require_ge,
    v2_require_n_accounts, v2_require_signer,
};

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

#[inline(always)]
pub fn process_hyper_commit_finalize(
    accounts: &[AccountView],
    data: &[u8],
) -> ProgramResult {
    //if data.len() != 0 {
    //    return Ok(());
    //}

    //let [
    //    validator, // force multi-line
    //    delegated_account,
    //] = v2_require_n_accounts!(accounts, 2);

    let validator = unsafe { accounts.get_unchecked(0) };
    let delegated_account = unsafe { accounts.get_unchecked(1) };

    let args = CommitFinalizeArgsWithBuffer::from_bytes(data)?;

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

        state_view.last_commit_id = args.commit_id;
        state_view.is_undelegatable = args.allow_undelegation.into();
    }

    delegated_account_data.copy_from_slice(args.buffer);

    Ok(())
}
