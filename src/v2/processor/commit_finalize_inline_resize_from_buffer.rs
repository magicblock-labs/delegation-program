use pinocchio::{AccountView, ProgramResult};

use crate::{
    pod_view::PodView,
    v2::{
        processor::internal::process_commit_finalize_inline_internal,
        CommitFinalizeArgs,
    },
};

#[inline(always)]
pub fn process_commit_finalize_inline_resize_from_buffer(
    accounts: &[AccountView],
    data: &[u8],
) -> ProgramResult {
    // let [
    //     validator, // force multi-line
    //     delegated_account,
    // ] = crate::v2_require_n_accounts!(accounts, 2);

    let validator = unsafe { accounts.get_unchecked(0) };
    let delegated_account = unsafe { accounts.get_unchecked(1) };
    let buffer_account = unsafe { accounts.get_unchecked(2) };
    let _system_program = unsafe { accounts.get_unchecked(3) };

    let args = CommitFinalizeArgs::try_view_from(data)?;

    let state_or_diff = unsafe { buffer_account.borrow_unchecked() };

    if args.data_is_diff.is_true() {
        process_commit_finalize_inline_internal::<true>(
            validator,
            delegated_account,
            args,
            state_or_diff,
        )
    } else {
        process_commit_finalize_inline_internal::<false>(
            validator,
            delegated_account,
            args,
            state_or_diff,
        )
    }
}
