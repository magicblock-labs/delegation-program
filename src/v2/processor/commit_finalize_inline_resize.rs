use std::ops::Deref;

use pinocchio::{error::ProgramError, AccountView, ProgramResult};

use crate::{
    processor::fast::NewState,
    v2::{
        processor::internal::{
            process_commit_finalize_inline_internal,
            DELEGATION_STATE_INLINE_SIZE,
        },
        CommitFinalizeArgsWithBuffer,
    },
    DiffSet,
};

#[inline(always)]
pub fn process_commit_finalize_inline_resize(
    accounts: &[AccountView],
    //data: &[u8],
    ixdata: *const u8,
    ixdatalen: usize,
) -> ProgramResult {
    // let [
    //     validator, // force multi-line
    //     delegated_account,
    // ] = crate::v2_require_n_accounts!(accounts, 2);

    //crate::v2_require_eq!(accounts.len(), 2, ProgramError::Immutable);

    let validator = unsafe { accounts.get_unchecked(0) };
    let delegated_account = unsafe { accounts.get_unchecked(1) };
    //let _system_program = unsafe { accounts.get_unchecked(2) };

    //let args = CommitFinalizeArgsWithBuffer::from_bytes(data)?;
    let args = CommitFinalizeArgsWithBuffer::from_bytes_ptr(ixdata, ixdatalen)?;

    if args.data_is_diff.is_true() {
        //let new_len = NewState::Diff(DiffSet::try_new(args.buffer)?).data_len();
        //delegated_account.resize(DELEGATION_STATE_INLINE_SIZE + new_len)?;

        process_commit_finalize_inline_internal::<true>(
            validator,
            delegated_account,
            args.deref(),
            args.buffer,
        )
    } else {
        // additional cost: 11 CU
        {
            let required_len = DELEGATION_STATE_INLINE_SIZE + args.buffer.len();
            if required_len != delegated_account.data_len() {
                delegated_account.resize(required_len)?;
            }
        }

        process_commit_finalize_inline_internal::<false>(
            validator,
            delegated_account,
            args.deref(),
            args.buffer,
        )
    }
}
