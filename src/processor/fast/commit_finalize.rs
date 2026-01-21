use pinocchio::{
    account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey, ProgramResult,
};
use pinocchio_log::log;

use crate::args::CommitFinalizeArgsWithBuffer;
use crate::processor::fast::commit_finalize_internal::{
    process_commit_finalize_internal, CommitFinalizeInternalArgs,
};
use crate::processor::fast::NewState;
use crate::{require_n_accounts, DiffSet};

/// Commit a new state of a delegated PDA
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
///
/// Instruction Data: CommitFinalizeArgsWithBuffer
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
pub fn process_commit_finalize(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let [
        validator, // force multi-line
        delegated_account,
        delegation_record_account,
        delegation_metadata_account,
        validator_fees_vault,
        program_config_account,
        _system_program,
    ] = require_n_accounts!(accounts, 7);

    let args = CommitFinalizeArgsWithBuffer::from_bytes(data)?;

    let commit_args = CommitFinalizeInternalArgs {
        delegation_record_bump: args.delegation_record_bump,
        delegation_metadata_bump: args.delegation_metadata_bump,
        validator_fees_vault_bump: args.validator_fees_vault_bump,
        program_config_bump: args.program_config_bump,
        new_state: match args.data_is_diff {
            0 => NewState::FullBytes(&args.buffer),
            1 => {
                let diffset = DiffSet::try_new(&args.buffer)?;
                if diffset.segments_count() == 0 {
                    log!("WARN: noop; empty diff sent");
                }
                NewState::Diff(diffset)
            }
            _ => return Err(ProgramError::InvalidInstructionData),
        },
        commit_id: args.commit_id,
        allow_undelegation: args.allow_undelegation == 1,
        validator,
        delegated_account,
        delegation_record_account,
        delegation_metadata_account,
        validator_fees_vault,
        program_config_account,
    };

    process_commit_finalize_internal(commit_args)
}
