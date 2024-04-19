use solana_program::{
    {self},
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    system_program,
};
use solana_program::program_error::ProgramError;

use crate::consts::{AUTHORITY, BUFFER};
use crate::instruction::DelegateArgs;
use crate::loaders::{load_owned_pda, load_program, load_signer, load_uninitialized_pda};
use crate::utils::create_pda;

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
    let args = DelegateArgs::try_from_bytes(data)?;
    let [pda, owner_program, buffer, authority_record, authority, system_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    load_program(system_program, system_program::id())?;
    load_owned_pda(pda, owner_program.key)?;
    load_uninitialized_pda(buffer, &[BUFFER, &pda.key.to_bytes()], args.buffer_bump, &crate::id())?;
    load_uninitialized_pda(authority_record, &[AUTHORITY, &pda.key.to_bytes()], args.authority_bump, &crate::id())?;
    load_signer(authority)?;
    // Initialize the buffer PDA
    create_pda(
        buffer,
        &crate::id(),
        pda.data_len(),
        &[BUFFER, &pda.key.to_bytes(), &[args.buffer_bump]],
        system_program,
        authority,
    )?;
    // TODO: Implement delegation logic
    //let mut buffer_data = buffer.try_borrow_mut_data()?;
    //let new_data = pda.try_borrow_data()?;
    //(*buffer_data).copy_from_slice(&new_data);
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
