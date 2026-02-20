use bytemuck::{Pod, Zeroable};
use pinocchio::{AccountView, ProgramResult};
use pinocchio_log::log;

use crate::{
    args::Boolean,
    processor::fast::NewState,
    v2::{
        processor::internal::{
            process_commit_finalize_internal, CommitFinalizeInternalArgs,
        },
        HeaderWithBuffer,
    },
    v2_require_n_accounts, DiffSet,
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

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct CommitFinalizeArgs {
    /// the commit_id ensures correct ordering of commits
    pub commit_id: u64,

    /// the lamports that the delegated account holds in the ephemeral validator
    pub lamports: u64,

    /// whether the account can be undelegated after the commit completes
    pub allow_undelegation: Boolean,

    /// whether the data (in the ixdata or in the data account) is diff or full state.
    pub data_is_diff: Boolean,

    pub reserved_padding: [u8; 6],
}

/// buffer is the diff-data or the full-bytes data
pub type CommitFinalizeArgsWithBuffer<'a> =
    HeaderWithBuffer<'a, CommitFinalizeArgs>;

pub fn process_commit_finalize(
    accounts: &[AccountView],
    data: &[u8],
) -> ProgramResult {
    let [
        validator, // force multi-line
        delegated_account,
        delegation_state,
        _system_program,
    ] = v2_require_n_accounts!(accounts, 4);

    let args = CommitFinalizeArgsWithBuffer::from_bytes(data)?;

    if args.data_is_diff.is_true() {
        let commit_args = CommitFinalizeInternalArgs {
            new_state: {
                let diffset = DiffSet::try_new(args.buffer)?;
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
            new_state: NewState::FullBytes(args.buffer),
            commit_id: args.commit_id,
            allow_undelegation: args.allow_undelegation.is_true(),
            validator,
            delegated_account,
            delegation_state,
        };
        process_commit_finalize_internal(commit_args)
    }
}
