use pinocchio::{AccountView, Address, ProgramResult};
use pinocchio_log::log;

use crate::{
    args::CommitFinalizeArgs,
    pod_view::PodView,
    processor::fast::{
        internal::{
            process_commit_finalize_internal, CommitFinalizeInternalArgs,
        },
        NewState,
    },
    require_n_accounts, DiffSet,
};

/// Commit a new state, or a diff, from a buffer account directly to the delegated account.
///
/// Accounts:
///
/// 0: `[signer]`   the validator requesting the commit
/// 1: `[]`         the delegated account
/// 2: `[]`         the delegation record
/// 3: `[writable]` the delegation metadata
/// 4: `[]`         the buffer account holding full state or diff bytes
/// 5: `[]`         the validator fees vault
/// 6: `[]`         the program config account
/// 7: `[]`         system program
///
/// Instruction Data: CommitFinalizeArgs
///
/// Requirements:
///
/// - delegation record is initialized
/// - delegation metadata is initialized
/// - validator fees vault is initialized
/// - if a program config PDA exists for the delegated account's owner program, the validator
///   must be whitelisted in that config (same rules as [`super::process_commit_state`])
/// - delegated account holds at least the lamports indicated in the delegation record

pub fn process_commit_finalize_from_buffer(
    _program_id: &Address,
    accounts: &[AccountView],
    data: &[u8],
) -> ProgramResult {
    let [
        validator, // force multi-line
        delegated_account,
        delegation_record_account,
        delegation_metadata_account,
        data_account, // full bytes or diff 
        validator_fees_vault,
        program_config_account,
        _system_program,
    ] = require_n_accounts!(accounts, 8);

    let args = CommitFinalizeArgs::try_view_from(data)?;

    let data = data_account.try_borrow()?;
    let commit_args = CommitFinalizeInternalArgs {
        bumps: &args.bumps,
        new_state: if args.data_is_diff.is_true() {
            let diffset = DiffSet::try_new(data.as_ref())?;
            if diffset.segments_count() == 0 {
                log!("WARN: noop; empty diff sent");
            }
            NewState::Diff(diffset)
        } else {
            NewState::FullBytes(&data)
        },
        commit_id: args.commit_id,
        allow_undelegation: args.allow_undelegation.is_true(),
        validator,
        delegated_account,
        delegation_record_account,
        delegation_metadata_account,
        validator_fees_vault,
        program_config_account,
    };

    process_commit_finalize_internal(commit_args)
}
