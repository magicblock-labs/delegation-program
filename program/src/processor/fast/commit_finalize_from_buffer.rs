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

/// Commit a new state of a delegated PDA
///
/// Accounts:
///
/// 0: `[signer]`   the validator requesting the commit
/// 1: `[writable]` the delegated account
/// 2: `[writable]` the delegation record
/// 3: `[writable]` the delegation metadata
/// 4: `[]`         the data buffer
/// 5: `[writable]` the validator fees vault
/// 6: `[]`         system program
///
/// Instruction Data: CommitFinalizeArgs
///
/// Requirements:
///
/// - delegation record is initialized
/// - delegation metadata is initialized
/// - validator fees vault is initialized
/// - program config is initialized
/// - commit state is uninitialized
/// - commit record is uninitialized
/// - delegated account holds at least the lamports indicated in the delegation record
/// - account was not committed at a later slot
///
/// Steps:
/// 1. Check that the pda is delegated
/// 2. Init a new PDA to store the new state
/// 3. Copy the new state to the new PDA
/// 4. Init a new PDA to store the record of the new state commitment
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
        _system_program,
    ] = require_n_accounts!(accounts, 7);

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
        commit_lamports: args.lamports,
        commit_id: args.commit_id,
        allow_undelegation: args.allow_undelegation.is_true(),
        validator,
        delegated_account,
        delegation_record_account,
        delegation_metadata_account,
        validator_fees_vault,
    };

    process_commit_finalize_internal(commit_args)
}
