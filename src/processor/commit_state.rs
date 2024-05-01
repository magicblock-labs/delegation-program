use solana_program::clock::Clock;
use solana_program::instruction::{AccountMeta, Instruction};
use solana_program::program::invoke;
use solana_program::program_error::ProgramError;
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    pubkey::Pubkey,
    system_program, {self},
};
use std::mem::size_of;

use crate::consts::{BUFFER, COMMIT_RECORD, DELEGATION, STATE_DIFF};
use crate::loaders::{
    load_initialized_pda, load_owned_pda, load_program, load_signer, load_uninitialized_pda,
};
use crate::state::{CommitState, Delegation};
use crate::utils::create_pda;
use crate::utils::{AccountDeserialize, Discriminator};
use solana_program::sysvar::Sysvar;

/// Commit a new state of a delegated Pda
///
/// 1. Check that the pda is delegated
/// 2. Init a new PDA to store the new state
/// 3. Copy the new state to the new PDA
/// 4. Init a new PDA to store the record of the new state commitment
/// 5. Increase the commits counter in the delegation record
///
pub fn process_commit_state<'a, 'info>(
    _program_id: &Pubkey,
    accounts: &'a [AccountInfo<'info>],
    data: &[u8],
) -> ProgramResult {
    msg!("Processing commit state instruction");
    msg!("Data: {:?}", data);
    let [authority, origin_account, new_state, commit_state_record, delegation_record, system_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    // Check that the origin account is delegated
    load_owned_pda(origin_account, &crate::id())?;
    load_signer(authority)?;
    load_initialized_pda(
        delegation_record,
        &[DELEGATION, &origin_account.key.to_bytes()],
        &crate::id(),
        true,
    )?;

    let mut delegation_record = delegation_record.try_borrow_mut_data()?;
    let delegation_record = Delegation::try_from_bytes_mut(&mut delegation_record)?;

    // Load the uninitialized PDAs
    let state_diff_bump = load_uninitialized_pda(
        new_state,
        &[STATE_DIFF, &origin_account.key.to_bytes()],
        &crate::id(),
    )?;
    let commit_state_bump = load_uninitialized_pda(
        commit_state_record,
        &[
            COMMIT_RECORD,
            &delegation_record.commits.to_be_bytes(),
            &origin_account.key.to_bytes(),
        ],
        &crate::id(),
    )?;

    // Initialize the PDA containing the new committed state
    create_pda(
        new_state,
        &crate::id(),
        data.len(),
        &[
            STATE_DIFF,
            &origin_account.key.to_bytes(),
            &[state_diff_bump],
        ],
        system_program,
        authority,
    )?;

    // Initialize the PDA containing the record of the committed state
    create_pda(
        commit_state_record,
        &crate::id(),
        8 + size_of::<CommitState>(),
        &[
            COMMIT_RECORD,
            &delegation_record.commits.to_be_bytes(),
            &origin_account.key.to_bytes(),
            &[commit_state_bump],
        ],
        system_program,
        authority,
    )?;

    let mut commit_record_data = commit_state_record.try_borrow_mut_data()?;
    commit_record_data[0] = CommitState::discriminator() as u8;
    let commit_record = CommitState::try_from_bytes_mut(&mut commit_record_data)?;
    commit_record.identity = *authority.key;
    commit_record.account = *origin_account.key;
    commit_record.timestamp = Clock::get()?.unix_timestamp;

    // TODO: here we can add a stake deposit to the commit record

    // Copy the new state to the initialized PDA
    let mut buffer_data = new_state.try_borrow_mut_data()?;
    (*buffer_data).copy_from_slice(data);

    // Increase the number of commits in the delegation record
    delegation_record.commits += 1;

    Ok(())
}