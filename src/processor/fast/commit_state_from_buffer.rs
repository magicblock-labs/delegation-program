use crate::args::CommitStateFromBufferArgs;
use crate::processor::fast::{process_commit_state_internal, CommitStateInternalArgs};

use borsh::BorshDeserialize;
use pinocchio::account_info::AccountInfo;
use pinocchio::program_error::ProgramError;
use pinocchio::pubkey::Pubkey;
use pinocchio::ProgramResult;

pub fn process_commit_state_from_buffer(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let args =
        CommitStateFromBufferArgs::try_from_slice(data).map_err(|_| ProgramError::BorshIoError)?;

    let commit_record_lamports = args.lamports;
    let commit_record_nonce = args.nonce;
    let allow_undelegation = args.allow_undelegation;

    let [validator, delegated_account, commit_state_account, commit_record_account, delegation_record_account, delegation_metadata_account, state_buffer_account, validator_fees_vault, program_config_account, system_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let state = state_buffer_account.try_borrow_data()?;
    let commit_state_bytes: &[u8] = &state;

    let commit_args = CommitStateInternalArgs {
        commit_state_bytes,
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
        system_program,
    };
    process_commit_state_internal(commit_args)
}
