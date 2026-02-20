use pinocchio::{AccountView, ProgramResult};
use pinocchio_log::log;

use crate::{
    pod_view::PodView,
    processor::fast::NewState,
    require_n_accounts,
    v2::{
        processor::internal::{
            process_commit_finalize_internal, CommitFinalizeInternalArgs,
        },
        CommitFinalizeArgs,
    },
    DiffSet,
};

/// Just like CommitFinalize, commit a new state, or a diff, directly to the delegated account. Unlike CommitFinalize, the state or diff comes from a buffer account.
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
/// Instruction Data: CommitFinalizeArgs
///
pub fn process_commit_finalize_from_buffer(
    accounts: &[AccountView],
    data: &[u8],
) -> ProgramResult {
    let [
        validator, // force multi-line
        delegated_account,
        delegation_state,
        buffer_account,
        _system_program,
    ] = require_n_accounts!(accounts, 5);

    let args = CommitFinalizeArgs::try_view_from(data)?;

    let data = buffer_account.try_borrow()?;

    if args.data_is_diff.is_true() {
        let commit_args = CommitFinalizeInternalArgs {
            new_state: {
                let diffset = DiffSet::try_new(&data)?;
                if diffset.segments_count() == 0 {
                    log!("WARN: noop; empty diff sent");
                }
                NewState::Diff(diffset)
            },
            commit_id: args.commit_id,
            allow_undelegation: args.allow_undelegation.is_true(),
            validator,
            delegated_account,
            delegation_state,
        };
        process_commit_finalize_internal(commit_args)
    } else {
        let commit_args = CommitFinalizeInternalArgs {
            new_state: NewState::FullBytes(&data),
            commit_id: args.commit_id,
            allow_undelegation: args.allow_undelegation.is_true(),
            validator,
            delegated_account,
            delegation_state,
        };
        process_commit_finalize_internal(commit_args)
    }
}
