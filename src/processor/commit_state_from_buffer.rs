use crate::args::CommitStateFromBufferArgs;
use crate::processor::{process_commit_state_internal, CommitStateInternalArgs};
use borsh::BorshDeserialize;
use solana_program::program_error::ProgramError;
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey};

/// Commit a new state of a delegated Pda
///
/// 1. Check that the pda is delegated
/// 2. Init a new PDA to store the new state
/// 3. Copy the new state to the new PDA
/// 4. Init a new PDA to store the record of the new state commitment
///
pub fn process_commit_state_from_buffer(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let args = CommitStateFromBufferArgs::try_from_slice(data)?;

    let commit_record_lamports = args.lamports;
    let commit_record_slot = args.slot;
    let allow_undelegation = args.allow_undelegation;

    let [validator, delegated_account, commit_state_account, commit_record_account, delegation_record_account, delegation_metadata_account, state_buffer_account, validator_fees_vault, program_config_account, system_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let state = state_buffer_account.try_borrow_data()?;
    let commit_state_bytes: &[u8] = *state;

    let commit_args = CommitStateInternalArgs {
        commit_state_bytes,
        commit_record_lamports,
        commit_record_slot,
        allow_undelegation,
        validator,
        delegated_account,
        commit_state_account,
        commit_record_account,
        delegation_record_account,
        delegation_metadata_account,
        validator_fees_vault,
        program_config_account,
        system_program,
    };
    process_commit_state_internal(commit_args)
}
