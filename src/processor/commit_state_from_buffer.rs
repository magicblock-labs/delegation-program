use crate::args::CommitStateFromBufferArgs;
use crate::processor::{process_commit_state_internal, CommitStateInternalArgs};
use borsh::BorshDeserialize;
use solana_program::program_error::ProgramError;
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey};

/// Commit a new state of a delegated Pda
///
/// It is identical to [crate::processor::process_commit_state] but it takes the new state
/// from a buffer account
///
/// Accounts:
///
/// 0: `[signer]`   the validator requesting the commit
/// 1: `[]`         the delegated account
/// 2: `[writable]` the PDA storing the new state temporarily
/// 3: `[writable]` the PDA storing the commit record
/// 4: `[]`         the delegation record
/// 5: `[writable]` the delegation metadata
/// 6: `[]`         the buffer account storing the data to be committed
/// 7: `[]`         the validator fees vault
/// 8: `[]`         the program config account
/// 9: `[]`         the system program
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
/// 1. Check that the pda is delegated
/// 2. Init a new PDA to store the new state
/// 3. Copy the new state to the new PDA
/// 4. Init a new PDA to store the record of the new state commitment
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
