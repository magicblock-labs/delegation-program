use solana_program::program_error::ProgramError;
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey, system_program, {self}, msg};
use std::mem::size_of;

use crate::consts::{BUFFER, DELEGATION};
use crate::instruction::DelegateArgs;
use crate::loaders::{load_owned_pda, load_program, load_signer, load_uninitialized_pda};
use crate::state::Delegation;
use crate::utils::create_pda;
use crate::utils::{AccountDeserialize, Discriminator};

/// Delegate a Pda to an authority
///
/// 1. Copy origin to a buffer PDA
/// 2. Close origin
/// 3. Reopen origin with authority set to the delegation program
/// 4. Save new authority in the Authority Record
///
/// Accounts expected: Origin PDA, Buffer PDA, Authority Record
pub fn process_delegate<'a, 'info>(
    _program_id: &Pubkey,
    accounts: &'a [AccountInfo<'info>],
    data: &[u8],
) -> ProgramResult {
    msg!("Processing delegate instruction");
    msg!("Data: {:?}", data);
    let args = DelegateArgs::try_from_bytes(data)?;
    msg!("Deserialize delegate args: {:?}", args);
    let [payer, pda, owner_program, buffer, delegation_record, new_authority, system_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    msg!("Load accounts");
    load_program(system_program, system_program::id())?;
    load_owned_pda(pda, owner_program.key)?;
    load_uninitialized_pda(
        buffer,
        &[BUFFER, &pda.key.to_bytes()],
        args.buffer_bump,
        &crate::id(),
    )?;
    load_uninitialized_pda(
        delegation_record,
        &[DELEGATION, &pda.key.to_bytes()],
        args.authority_bump,
        &crate::id(),
    )?;
    load_signer(payer)?;
    msg!("Create PDAs and initialize delegation record");
    // Initialize the buffer PDA
    create_pda(
        buffer,
        &crate::id(),
        pda.data_len(),
        &[BUFFER, &pda.key.to_bytes(), &[args.buffer_bump]],
        system_program,
        payer,
    )?;
    // // Initialize the delegation record PDA
    // create_pda(
    //     delegation_record,
    //     &crate::id(),
    //     8 + size_of::<Delegation>(),
    //     &[DELEGATION, &pda.key.to_bytes(), &[args.authority_bump]],
    //     system_program,
    //     payer,
    // )?;
    // // 1. Copy the date to the buffer PDA
    // let mut buffer_data = buffer.try_borrow_mut_data()?;
    // let new_data = pda.try_borrow_data()?;
    // (*buffer_data).copy_from_slice(&new_data);
    // // 2. CPI into the owner program to Close the PDA
    // // TODO: Implement close logic in an external program and call it here
    // // 3. CPI into the owner program to re-init the PDA, setting the authority to the delegation program
    // // TODO: Implement init logic in an external program and call it here
    // // 4. Save new delegation in the Delegation Record
    // let mut delegation_data = delegation_record.try_borrow_mut_data()?;
    // delegation_data[0] = Delegation::discriminator() as u8;
    // let delegation = Delegation::try_from_bytes_mut(&mut delegation_data)?;
    // delegation.origin = *owner_program.key;
    // delegation.authority = *new_authority.key;
    // delegation.valid_until = 0;
    Ok(())
}

/// Update the data of a delegated Pda
///
/// 1. Copy delegated PDA to a buffer PDA
/// 2. Close PDA and reopen it with the origin authority
/// 3. Reopen origin with authority set to the delegation program
/// 4. Save new authority in the Authority Record
///
/// Accounts expected: Authority Record, Buffer PDA, Delegated PDA
pub fn process_update<'a, 'info>(
    _program_id: &Pubkey,
    accounts: &'a [AccountInfo<'info>],
    data: &[u8],
) -> ProgramResult {
    // TODO: Implement delegation logic

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
