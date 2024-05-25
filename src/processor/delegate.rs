use std::mem::size_of;

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::program_error::ProgramError;
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    system_program, {self},
};

use crate::consts::{BUFFER, DELEGATED_ACCOUNT_SEEDS, DELEGATION_RECORD};
use crate::instruction::DelegateAccountArgs;
use crate::loaders::{
    load_initialized_pda, load_owned_pda, load_program, load_signer, load_uninitialized_pda,
};
use crate::state::{DelegateAccountSeeds, DelegationRecord};
use crate::utils::create_pda;
use crate::utils_account::{AccountDeserialize, Discriminator};

/// Delegate an account
///
/// 1. Checks that the account is owned by the delegation program, that the buffer is initialized and derived correctly from the PDA
///  - Also checks that the delegate_account is a signer (enforcing that the instruction is being called from CPI) & other constraints
/// 2. Copy the data from the buffer into the original account
/// 3. Create a Delegation Record to store useful information about the delegation event
/// 4. Create a Delegated Account Seeds to store the seeds used to derive the delegate account. Needed for undelegation.
///
pub fn process_delegate(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let [payer, delegate_account, owner_program, buffer, delegation_record, delegate_account_seeds, system_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    let args = DelegateAccountArgs::try_from_slice(data)?;

    load_program(system_program, system_program::id())?;
    load_owned_pda(delegate_account, &crate::id())?;

    // Validate the seeds
    let seeds_to_validate: Vec<&[u8]> = args.seeds.iter().map(|v| v.as_slice()).collect();
    let (derived_pda, _) =
        Pubkey::find_program_address(seeds_to_validate.as_ref(), owner_program.key);
    if derived_pda.ne(delegate_account.key) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Check that the buffer PDA is initialized and derived correctly from the PDA
    load_initialized_pda(
        buffer,
        &[BUFFER, &delegate_account.key.to_bytes()],
        owner_program.key,
        false,
    )?;

    // Check that the delegation record PDA is uninitialized
    let delegation_record_bump = load_uninitialized_pda(
        delegation_record,
        &[DELEGATION_RECORD, &delegate_account.key.to_bytes()],
        &crate::id(),
    )?;

    let delegate_account_seeds_bump = load_uninitialized_pda(
        delegate_account_seeds,
        &[DELEGATED_ACCOUNT_SEEDS, &delegate_account.key.to_bytes()],
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
            &[delegation_record_bump],
        ],
        system_program,
        payer,
    )?;

    // Initialize the delegation record
    let mut delegation_data = delegation_record.try_borrow_mut_data()?;
    delegation_data[0] = DelegationRecord::discriminator() as u8;
    let delegation = DelegationRecord::try_from_bytes_mut(&mut delegation_data)?;
    delegation.owner = *owner_program.key;
    delegation.authority = Pubkey::default();
    delegation.valid_until = args.valid_until;
    delegation.commit_frequency_ms = args.commit_frequency_ms as u64;

    // Initialize the account seeds PDA
    create_pda(
        delegate_account_seeds,
        &crate::id(),
        size_of::<DelegateAccountSeeds>(),
        &[
            DELEGATED_ACCOUNT_SEEDS,
            &delegate_account.key.to_bytes(),
            &[delegate_account_seeds_bump],
        ],
        system_program,
        payer,
    )?;

    // Copy the seeds to the delegated account seeds PDA
    let seeds_struct = DelegateAccountSeeds { seeds: args.seeds };
    seeds_struct.serialize(&mut &mut delegate_account_seeds.try_borrow_mut_data()?.as_mut())?;

    // Copy the data from the buffer into the original account
    let mut account_data = delegate_account.try_borrow_mut_data()?;
    let new_data = buffer.try_borrow_data()?;
    (*account_data).copy_from_slice(&new_data);

    Ok(())
}
