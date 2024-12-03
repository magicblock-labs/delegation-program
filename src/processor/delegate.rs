use std::mem::size_of;

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::program_error::ProgramError;
use solana_program::sysvar::Sysvar;
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    system_program, {self},
};

use crate::consts::{BUFFER, DELEGATION_METADATA, DELEGATION_RECORD};
use crate::processor::utils::loaders::{
    load_owned_pda, load_pda, load_program, load_signer, load_uninitialized_pda,
};
use crate::processor::utils::pda::{create_pda, ValidateEdwards};
use crate::state::account::{AccountDeserialize, Discriminator};
use crate::state::{DelegationMetadata, DelegationRecord};

#[derive(Default, Debug, BorshSerialize, BorshDeserialize)]
pub struct DelegateArgs {
    pub valid_until: i64,
    pub commit_frequency_ms: u32,
    pub seeds: Vec<Vec<u8>>,
    pub validator: Option<Pubkey>,
}

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
    let [payer, delegate_account, owner_program, buffer, delegation_record, delegation_metadata, system_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    let args = DelegateArgs::try_from_slice(data)?;

    load_program(system_program, system_program::id())?;
    load_owned_pda(delegate_account, &crate::id())?;

    // Validate seeds if the delegate account is not on curve, i.e. is a PDA
    if !delegate_account.is_on_curve() {
        let seeds_to_validate: Vec<&[u8]> = args.seeds.iter().map(|v| v.as_slice()).collect();
        let (derived_pda, _) =
            Pubkey::find_program_address(seeds_to_validate.as_ref(), owner_program.key);
        if derived_pda.ne(delegate_account.key) {
            return Err(ProgramError::InvalidSeeds);
        }
    }

    // Check that the buffer PDA is initialized and derived correctly from the PDA
    load_pda(
        buffer,
        &[BUFFER, &delegate_account.key.to_bytes()],
        owner_program.key,
        true,
    )?;

    // Check that the delegation record PDA is uninitialized
    let delegation_record_bump = load_uninitialized_pda(
        delegation_record,
        &[DELEGATION_RECORD, &delegate_account.key.to_bytes()],
        &crate::id(),
    )?;

    // Check that the delegation metadata PDA is uninitialized
    let delegation_metadata_bump = load_uninitialized_pda(
        delegation_metadata,
        &[DELEGATION_METADATA, &delegate_account.key.to_bytes()],
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
    delegation.authority = args.validator.unwrap_or(Pubkey::default());
    delegation.commit_frequency_ms = args.commit_frequency_ms as u64;
    delegation.delegation_slot = solana_program::clock::Clock::get()?.slot;
    delegation.lamports = delegate_account.lamports();

    // Initialize the account seeds PDA
    let delegation_metadata_struct = DelegationMetadata {
        seeds: args.seeds,
        valid_until: args.valid_until,
        last_update_external_slot: 0,
        is_undelegatable: false,
        rent_payer: *payer.key,
    };

    let serialized_metadata_struct = delegation_metadata_struct.try_to_vec()?;
    create_pda(
        delegation_metadata,
        &crate::id(),
        serialized_metadata_struct.len(),
        &[
            DELEGATION_METADATA,
            &delegate_account.key.to_bytes(),
            &[delegation_metadata_bump],
        ],
        system_program,
        payer,
    )?;

    // Copy the seeds to the delegated account seeds PDA
    let mut seeds_data = delegation_metadata.try_borrow_mut_data()?;
    (*seeds_data).copy_from_slice(serialized_metadata_struct.as_slice());

    // Copy the data from the buffer into the original account
    if !buffer.data_is_empty() {
        let mut account_data = delegate_account.try_borrow_mut_data()?;
        let new_data = buffer.try_borrow_data()?;
        (*account_data).copy_from_slice(&new_data);
    }

    Ok(())
}
