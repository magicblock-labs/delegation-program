use std::mem::size_of;

use solana_program::program_error::ProgramError;
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    system_program, {self},
};

use crate::consts::{BUFFER, DELEGATION_RECORD};
use crate::instruction::DelegateArgs;
use crate::loaders::{
    load_initialized_pda, load_owned_pda, load_program, load_signer, load_uninitialized_pda,
};
use crate::state::DelegationRecord;
use crate::utils::create_pda;
use crate::utils::{AccountDeserialize, Discriminator};

/// Delegate an account
///
/// 1. Checks that the account is owned by the delegation program, that the buffer is initialized and derived correctly from the PDA
///  - Also checks that the delegate_account is a signer (enforcing that the instruction is being called from CPI) & other constraints
/// 2. Copy the data from the buffer into the original account
/// 3. Create a Delegation Record to store useful information about the delegation event
///
pub fn process_delegate(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let [payer, delegate_account, owner_program, buffer, delegation_record, system_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    let default_args = DelegateArgs::default();
    let args = DelegateArgs::try_from_bytes(data).unwrap_or(&default_args);

    load_program(system_program, system_program::id())?;
    load_owned_pda(delegate_account, &crate::id())?;

    // Check that the buffer PDA is initialized and derived correctly from the PDA
    load_initialized_pda(
        buffer,
        &[BUFFER, &delegate_account.key.to_bytes()],
        owner_program.key,
        false,
    )?;
    let authority_bump = load_uninitialized_pda(
        delegation_record,
        &[DELEGATION_RECORD, &delegate_account.key.to_bytes()],
        &crate::id(),
    )?;

    // Check that payer and delegate_account are signers, this ensures the instruction is being called from CPI
    load_signer(payer)?;
    load_signer(delegate_account)?;

    // Initialize the delegation record PDA
    create_pda(
        delegation_record,
        &crate::id(),
        8 + size_of::<DelegationRecord>(),
        &[
            DELEGATION_RECORD,
            &delegate_account.key.to_bytes(),
            &[authority_bump],
        ],
        system_program,
        payer,
    )?;

    // Copy the data from the buffer into the original account
    let mut account_data = delegate_account.try_borrow_mut_data()?;
    let new_data = buffer.try_borrow_data()?;
    (*account_data).copy_from_slice(&new_data);

    // Initialize the delegation record
    let mut delegation_data = delegation_record.try_borrow_mut_data()?;
    delegation_data[0] = DelegationRecord::discriminator() as u8;
    let delegation = DelegationRecord::try_from_bytes_mut(&mut delegation_data)?;
    delegation.owner = *owner_program.key;
    delegation.authority = Pubkey::default();
    delegation.valid_until = args.valid_until;
    delegation.commit_frequency_ms = args.update_frequency_ms;
    Ok(())
}
