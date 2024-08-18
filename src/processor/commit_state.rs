use std::mem::size_of;

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::program_error::ProgramError;
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    pubkey::Pubkey,
    {self},
};

use crate::consts::{COMMIT_RECORD, COMMIT_STATE, DELEGATION_METADATA, DELEGATION_RECORD};
use crate::instruction::CommitAccountArgs;
use crate::loaders::{load_initialized_pda, load_owned_pda, load_signer, load_uninitialized_pda};
use crate::state::{CommitRecord, DelegationMetadata};
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
    msg!("Processing CommitState");
    let args = CommitAccountArgs::try_from_slice(data)?;
    let data: &[u8] = args.data.as_ref();

    let [authority, delegated_account, commit_state_account, commit_state_record, delegation_record, delegation_metadata, system_program] =
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
    // Check that the delegation_metadata account
    load_owned_pda(delegation_metadata, &crate::id())?;
    load_initialized_pda(
        delegation_metadata,
        &[DELEGATION_METADATA, &delegated_account.key.to_bytes()],
        &crate::id(),
        true,
    )?;
    let mut delegation_metadata_data = delegation_metadata.try_borrow_mut_data()?;
    let mut delegation_metadata = DelegationMetadata::try_from_slice(&delegation_metadata_data)?;

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
    commit_record.slot = args.slot;

    delegation_metadata.is_undelegatable = args.allow_undelegation;
    delegation_metadata.serialize(&mut &mut delegation_metadata_data.as_mut())?;

    // Copy the new state to the initialized PDA
    let mut buffer_data = commit_state_account.try_borrow_mut_data()?;
    (*buffer_data).copy_from_slice(data);

    Ok(())
}
