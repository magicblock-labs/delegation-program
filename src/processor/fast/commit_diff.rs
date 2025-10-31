use borsh::BorshDeserialize;
use pinocchio::{
    account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey, ProgramResult,
};
use pinocchio_log::log;

use crate::args::{CommitDiffArgsWithoutDiff, SIZE_COMMIT_DIFF_ARGS_WITHOUT_DIFF};
use crate::processor::fast::{process_commit_state_internal, CommitStateInternalArgs};
use crate::{apply_diff_copy, DiffSet};

/// Commit diff to a delegated PDA
///
/// Accounts:
///
/// 0: `[signer]`   the validator requesting the commit
/// 1: `[]`         the delegated account
/// 2: `[writable]` the PDA storing the new state
/// 3: `[writable]` the PDA storing the commit record
/// 4: `[]`         the delegation record
/// 5: `[writable]` the delegation metadata
/// 6: `[]`         the validator fees vault
/// 7: `[]`         the program config account
/// 8: `[]`         the system program
///
/// Requirements:
///
/// - The following accounts must be initialized:
///   - delegation record
///   - delegation metadata
///   - validator fees vault
///   - program config
/// - The following accounts must be uninitialized:
///   - commit state
///   - commit record
/// - delegated account holds at least the lamports indicated in the delegation record
/// - account was not committed at a later slot
///
/// Steps:
/// 1. Check that the pda is delegated
/// 2. Init a new PDA to store the new state
/// 3. Copy the new state to the new PDA
/// 4. Init a new PDA to store the record of the new state commitment
pub fn process_commit_diff(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let [validator, delegated_account, commit_state_account, commit_record_account, delegation_record_account, delegation_metadata_account, validator_fees_vault, program_config_account, _system_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if data.len() < SIZE_COMMIT_DIFF_ARGS_WITHOUT_DIFF {
        return Err(ProgramError::InvalidInstructionData);
    }

    let (diff, data) = data.split_at(data.len() - SIZE_COMMIT_DIFF_ARGS_WITHOUT_DIFF);

    let args =
        CommitDiffArgsWithoutDiff::try_from_slice(data).map_err(|_| ProgramError::BorshIoError)?;

    let diffset = DiffSet::try_new_from_borsh_vec(diff)?;

    if diffset.segments_count() == 0 {
        log!("WARN: noop; empty diff sent");
    }

    let commit_record_lamports = args.lamports;
    let commit_record_nonce = args.nonce;
    let allow_undelegation = args.allow_undelegation;

    // TODO (snawaz): the following approach to apply diff works, but it's not efficient.
    // It is also problematic for larger account as it allocates memory on the heap.
    // It will be fixed in a separate PR.
    let original = unsafe { delegated_account.borrow_data_unchecked() };
    let changed = apply_diff_copy(original, &diffset)?;

    let commit_args = CommitStateInternalArgs {
        commit_state_bytes: &changed,
        commit_record_lamports,
        commit_record_nonce,
        allow_undelegation,
        validator,
        delegated_account,
        commit_state_account,
        commit_record_account,
        delegation_record_account,
        delegation_metadata_account,
        validator_fees_vault,
        program_config_account,
    };

    process_commit_state_internal(commit_args)
}
