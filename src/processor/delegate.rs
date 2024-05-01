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
use solana_program::clock::Clock;

use crate::consts::{BUFFER, COMMIT_RECORD, DELEGATION, STATE_DIFF};
use crate::loaders::{load_initialized_pda, load_owned_pda, load_program, load_signer, load_uninitialized_pda};
use crate::state::{CommitState, Delegation};
use crate::utils::create_pda;
use crate::utils::{AccountDeserialize, Discriminator};
use solana_program::sysvar::Sysvar;

/// Delegate a Pda to an authority
///
/// 1. Copy origin to a buffer PDA
/// 2. Close origin
/// 3. Reopen origin with authority set to the delegation program
/// 4. Save new authority in the Authority Record
///
pub fn process_delegate<'a, 'info>(
    _program_id: &Pubkey,
    accounts: &'a [AccountInfo<'info>],
    data: &[u8],
) -> ProgramResult {
    msg!("Processing delegate instruction");
    msg!("Data: {:?}", data);
    let [payer, pda, owner_program, buffer, delegation_record, new_authority, system_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    msg!("Load accounts");
    load_program(system_program, system_program::id())?;
    load_owned_pda(pda, owner_program.key)?;
    let buffer_bump = load_uninitialized_pda(buffer, &[BUFFER, &pda.key.to_bytes()], &crate::id())?;
    let authority_bump = load_uninitialized_pda(
        delegation_record,
        &[DELEGATION, &pda.key.to_bytes()],
        &crate::id(),
    )?;
    msg!("Payer is Signer: {:?}", payer.is_signer);
    msg!("Payer is Writable: {:?}", payer.is_writable);
    msg!("Buffer is Signer: {:?}", buffer.is_signer);
    msg!("Buffer is Writable: {:?}", buffer.is_writable);
    msg!("Buffer Address: {:?}", buffer.key.to_string());
    msg!("PDA is Signer: {:?}", pda.is_signer);
    msg!("PDA is Writable: {:?}", pda.is_writable);
    msg!("PDA Address: {:?}", pda.key.to_string());
    load_signer(payer)?;
    msg!("Create PDAs and initialize delegation record");

    // Initialize the buffer PDA
    create_pda(
        buffer,
        &crate::id(),
        pda.data_len(),
        &[BUFFER, &pda.key.to_bytes(), &[buffer_bump]],
        system_program,
        payer,
    )?;

    // Initialize the delegation record PDA
    create_pda(
        delegation_record,
        &crate::id(),
        8 + size_of::<Delegation>(),
        &[DELEGATION, &pda.key.to_bytes(), &[authority_bump]],
        system_program,
        payer,
    )?;

    // 1. Copy the date to the buffer PDA
    let mut buffer_data = buffer.try_borrow_mut_data()?;
    let new_data = pda.try_borrow_data()?;
    (*buffer_data).copy_from_slice(&new_data);
    // 2. CPI into the owner program to Close the PDA
    // TODO: Implement close logic in an external program and call it here with CPI to owner program
    //drop(new_data);
    //call_close_pda(pda, payer, owner_program.key)?;
    // 3. CPI into the owner program to re-init the PDA, setting the authority to the delegation program
    // TODO: Implement init logic in an external program and call it here with CPI to owner program
    // 4. Save new delegation in the Delegation Record
    let mut delegation_data = delegation_record.try_borrow_mut_data()?;
    delegation_data[0] = Delegation::discriminator() as u8;
    let delegation = Delegation::try_from_bytes_mut(&mut delegation_data)?;
    delegation.origin = *owner_program.key;
    delegation.authority = *new_authority.key;
    delegation.valid_until = 0;
    delegation.commits = 0;
    Ok(())
}

/// Update the data of a delegated Pda
///
/// 1. Copy delegated PDA to a buffer PDA
/// 2. Close PDA and reopen it with the origin authority
/// 3. Reopen origin with authority set to the delegation program
/// 4. Save new authority in the Authority Record
///
pub fn process_update<'a, 'info>(
    _program_id: &Pubkey,
    accounts: &'a [AccountInfo<'info>],
    data: &[u8],
) -> ProgramResult {
    // TODO: Implement delegation logic

    Ok(())
}

/// Commit a new state of a delegated Pda
///
/// 1. Check that the pda is delegated
/// 2. Init a new PDA to store the new state
/// 3. Copy the new state to the new PDA
///
/// TODO: Currently only one state can be committed for a delegated account.
/// In the future, we may want to allow multiple states to be committed, e.g. from different operators.
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
    load_signer(authority)?;
    load_initialized_pda(
        delegation_record,
        &[DELEGATION, &origin_account.key.to_bytes()],
        &crate::id(),
        true
    )?;

    let mut delegation_record = delegation_record.try_borrow_mut_data()?;
    let delegation_record = Delegation::try_from_bytes_mut(&mut delegation_record)?;


    let state_diff_bump = load_uninitialized_pda(new_state, &[STATE_DIFF, &origin_account.key.to_bytes()], &crate::id())?;
    let commit_state_bump = load_uninitialized_pda(commit_state_record, &[COMMIT_RECORD, &delegation_record.commits.to_be_bytes(),  &origin_account.key.to_bytes()], &crate::id())?;

    // Initialize the PDA containing the new committed state
    create_pda(
        new_state,
        &crate::id(),
        data.len(),
        &[STATE_DIFF, &origin_account.key.to_bytes(), &[state_diff_bump]],
        system_program,
        authority,
    )?;

    // Initialize the PDA containing the record of the committed state
    create_pda(
        commit_state_record,
        &crate::id(),
        data.len(),
        &[COMMIT_RECORD, &delegation_record.commits.to_be_bytes(), &origin_account.key.to_bytes(), &[commit_state_bump]],
        system_program,
        authority,
    )?;

    let mut commit_state_record_data = commit_state_record.try_borrow_mut_data()?;
    let commit_record = CommitState::try_from_bytes_mut(&mut commit_state_record_data)?;
    commit_record.identity = *authority.key;
    commit_record.account = *origin_account.key;
    commit_record.timestamp = Clock::get()?.unix_timestamp;

    // Copy the new state to the initialized PDA
    let mut buffer_data = new_state.try_borrow_mut_data()?;
    (*buffer_data).copy_from_slice(data);

    // Increase the number of commits in the delegation record
    delegation_record.commits += 1;

    Ok(())
}

/// Undelegate a delegated Pda
///
/// 1. Copy origin to a buffer PDA
/// 2. Close origin and reopen it with authority set to the delegation program
/// 3. Copy buffer PDA to the origin PDA and close the Authority Record
///
/// Accounts expected: Authority Record, Buffer PDA, Delegated PDA
pub fn process_undelegate<'a, 'info>(
    _program_id: &Pubkey,
    accounts: &'a [AccountInfo<'info>],
    data: &[u8],
) -> ProgramResult {
    // TODO: Implement delegation logic

    Ok(())
}

const CLOSE_INSTRUCTION_DISCRIMINATOR: [u8; 8] = [98, 165, 201, 177, 108, 65, 206, 96];

fn call_close_pda<'a, 'info>(
    account_to_close: &'a AccountInfo<'info>,
    destination_account: &'a AccountInfo<'info>,
    program_id: &Pubkey, // Anchor program's ID
) -> ProgramResult {
    let instruction_data = CLOSE_INSTRUCTION_DISCRIMINATOR.to_vec();

    let close_instruction = Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta {
                pubkey: *account_to_close.key,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: *account_to_close.key,
                is_signer: false,
                is_writable: true,
            },
        ],
        data: instruction_data,
    };

    invoke(
        &close_instruction,
        &[account_to_close.clone(), destination_account.clone()],
    )
}
