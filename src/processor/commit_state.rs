use std::mem::size_of;

use solana_program::clock::Clock;
use solana_program::program_error::ProgramError;
use solana_program::sysvar::Sysvar;
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    {self},
};

use crate::consts::{COMMIT_RECORD, COMMIT_STATE, DELEGATION_RECORD};
use crate::loaders::{load_initialized_pda, load_owned_pda, load_signer, load_uninitialized_pda};
use crate::state::CommitRecord;
use crate::utils::create_pda;
use crate::utils_account::{AccountDeserialize, Discriminator};

/// Commit a new state of a delegated Pda
///
/// 1. Check that the pda is delegated
/// 2. Init a new PDA to store the new state
/// 3. Copy the new state to the new PDA
/// 4. Init a new PDA to store the record of the new state commitment
/// 5. Increase the commits counter in the delegation record
///
pub fn process_commit_state(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let [authority, delegated_account, commit_state_account, commit_state_record, delegation_record, system_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Check that the origin account is delegated
    load_owned_pda(delegated_account, &crate::id())?;
    load_signer(authority)?;
    load_initialized_pda(
        delegation_record,
        &[DELEGATION_RECORD, &delegated_account.key.to_bytes()],
        &crate::id(),
        true,
    )?;
    //let mut delegation_record = delegation_record.try_borrow_mut_data()?;
    //let delegation_record = DelegationRecord::try_from_bytes_mut(&mut delegation_record)?;

    // Load the uninitialized PDAs
    let state_diff_bump = load_uninitialized_pda(
        commit_state_account,
        &[COMMIT_STATE, &delegated_account.key.to_bytes()],
        &crate::id(),
    )?;
    let commit_state_bump = load_uninitialized_pda(
        commit_state_record,
        &[COMMIT_RECORD, &delegated_account.key.to_bytes()],
        &crate::id(),
    )?;

    // Initialize the PDA containing the new committed state
    create_pda(
        commit_state_account,
        &crate::id(),
        data.len(),
        &[
            COMMIT_STATE,
            &delegated_account.key.to_bytes(),
            &[state_diff_bump],
        ],
        system_program,
        authority,
    )?;

    // Initialize the PDA containing the record of the committed state
    create_pda(
        commit_state_record,
        &crate::id(),
        8 + size_of::<CommitRecord>(),
        &[
            COMMIT_RECORD,
            &delegated_account.key.to_bytes(),
            &[commit_state_bump],
        ],
        system_program,
        authority,
    )?;

    let mut commit_record_data = commit_state_record.try_borrow_mut_data()?;
    commit_record_data[0] = CommitRecord::discriminator() as u8;
    let commit_record = CommitRecord::try_from_bytes_mut(&mut commit_record_data)?;
    commit_record.identity = *authority.key;
    commit_record.account = *delegated_account.key;
    commit_record.timestamp = Clock::get()?.unix_timestamp;

    // TODO: here we can add a stake deposit to the state commit record

    // Copy the new state to the initialized PDA
    let mut buffer_data = commit_state_account.try_borrow_mut_data()?;
    (*buffer_data).copy_from_slice(data);

    Ok(())
}
