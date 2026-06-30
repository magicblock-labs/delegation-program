use pinocchio::{AccountView, Address, ProgramResult};
use pinocchio_log::log;

use crate::{
    args::CommitFinalizeArgsWithBuffer,
    processor::fast::{
        internal::{
            parse_auto_undelegation_accounts,
            process_auto_undelegation_if_requested,
            process_commit_finalize_internal, CommitFinalizeInternalArgs,
        },
        NewState,
    },
    require_n_accounts_with_optionals, DiffSet,
};

/// Commit a new state, or a diff, directly to the delegated account. Unlike, CommitState and
/// CommitDiff variants, this instruction does not write to any temporary account first. In other
/// words, this instruction commits and finalizes both.
///
/// Accounts:
///
/// 0: `[signer]`   the validator requesting the commit
/// 1: `[writable]` the delegated account
/// 2: `[writable]` the delegation record
/// 3: `[writable]` the delegation metadata
/// 4: `[writable]` the validator fees vault
/// 5: `[]`         system program
///
/// Instruction Data: CommitFinalizeArgsWithBuffer
///
pub fn process_commit_finalize(
    _program_id: &Address,
    accounts: &[AccountView],
    data: &[u8],
) -> ProgramResult {
    let (
        [
            validator, // force multi-line
            delegated_account,
            delegation_record_account,
            delegation_metadata_account,
            validator_fees_vault,
            system_program,
        ],
        optional_accounts,
    ) = require_n_accounts_with_optionals!(accounts, 6);
    let auto_accounts = parse_auto_undelegation_accounts(optional_accounts)?;

    let args = CommitFinalizeArgsWithBuffer::from_bytes(data)?;

    let commit_args = CommitFinalizeInternalArgs {
        bumps: &args.bumps,
        new_state: if args.data_is_diff.is_true() {
            let diffset = DiffSet::try_new(args.buffer)?;
            if diffset.segments_count() == 0 {
                log!("WARN: noop; empty diff sent");
            }
            NewState::Diff(diffset)
        } else {
            NewState::FullBytes(args.buffer)
        },
        commit_lamports: args.lamports,
        commit_id: args.commit_id,
        allow_undelegation: args.allow_undelegation.is_true(),
        validator,
        delegated_account,
        delegation_record_account,
        delegation_metadata_account,
        validator_fees_vault,
    };

    let requester = process_commit_finalize_internal(commit_args)?;
    process_auto_undelegation_if_requested(
        requester,
        validator,
        delegated_account,
        delegation_record_account,
        delegation_metadata_account,
        validator_fees_vault,
        system_program,
        auto_accounts,
    )
}
