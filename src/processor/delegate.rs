use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::program_error::ProgramError;
use solana_program::sysvar::Sysvar;
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    system_program, {self},
};

use crate::args::DelegateArgs;
use crate::consts::{BUFFER, DELEGATION_METADATA, DELEGATION_RECORD};
use crate::processor::utils::curve::is_on_curve;
use crate::processor::utils::loaders::{
    load_owned_pda, load_pda, load_program, load_signer, load_uninitialized_pda,
};
use crate::processor::utils::pda::create_pda;
use crate::state::account::{AccountDeserialize, AccountWithDiscriminator};
use crate::state::{DelegationMetadata, DelegationRecord};

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
    let [payer, delegate_account, owner_program, buffer_account, delegation_record_account, delegation_metadata_account, system_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    let args = DelegateArgs::try_from_slice(data)?;

    load_owned_pda(delegate_account, &crate::id())?;
    load_program(system_program, system_program::id())?;

    // Validate seeds if the delegate account is not on curve, i.e. is a PDA
    if !is_on_curve(delegate_account.key) {
        let seeds_to_validate: Vec<&[u8]> = args.seeds.iter().map(|v| v.as_slice()).collect();
        let (derived_pda, _) =
            Pubkey::find_program_address(seeds_to_validate.as_ref(), owner_program.key);
        if derived_pda.ne(delegate_account.key) {
            return Err(ProgramError::InvalidSeeds);
        }
    }

    // Check that the buffer PDA is initialized and derived correctly from the PDA
    load_pda(
        buffer_account,
        &[BUFFER, &delegate_account.key.to_bytes()],
        owner_program.key,
        true,
    )?;

    // Check that the delegation record PDA is uninitialized
    let delegation_record_bump = load_uninitialized_pda(
        delegation_record_account,
        &[DELEGATION_RECORD, &delegate_account.key.to_bytes()],
        &crate::id(),
    )?;

    // Check that the delegation metadata PDA is uninitialized
    let delegation_metadata_bump = load_uninitialized_pda(
        delegation_metadata_account,
        &[DELEGATION_METADATA, &delegate_account.key.to_bytes()],
        &crate::id(),
    )?;

    // Check that payer and delegate_account are signers, this ensures the instruction is being called from CPI
    load_signer(payer)?;
    load_signer(delegate_account)?;

    // Initialize the delegation record PDA
    create_pda(
        delegation_record_account,
        &crate::id(),
        DelegationRecord::size_with_discriminant(),
        &[
            DELEGATION_RECORD,
            &delegate_account.key.to_bytes(),
            &[delegation_record_bump],
        ],
        system_program,
        payer,
    )?;

    // Initialize the delegation record
    let mut delegation_record_data = delegation_record_account.try_borrow_mut_data()?;
    delegation_record_data[0] = DelegationRecord::discriminator() as u8;
    let delegation_record = DelegationRecord::try_from_bytes_mut(&mut delegation_record_data)?;
    delegation_record.owner = *owner_program.key;
    delegation_record.authority = args.validator.unwrap_or(Pubkey::default());
    delegation_record.commit_frequency_ms = args.commit_frequency_ms as u64;
    delegation_record.delegation_slot = solana_program::clock::Clock::get()?.slot;
    delegation_record.lamports = delegate_account.lamports();

    // Initialize the account seeds PDA
    let delegation_metadata_struct = DelegationMetadata {
        seeds: args.seeds,
        valid_until: args.valid_until,
        last_update_external_slot: 0,
        is_undelegatable: false,
        rent_payer: *payer.key,
    };
    let delegation_metadata_serialized = delegation_metadata_struct.try_to_vec()?;

    // Initialize the delegation metadata PDA
    create_pda(
        delegation_metadata_account,
        &crate::id(),
        delegation_metadata_serialized.len(),
        &[
            DELEGATION_METADATA,
            &delegate_account.key.to_bytes(),
            &[delegation_metadata_bump],
        ],
        system_program,
        payer,
    )?;

    // Copy the seeds to the delegated metadata PDA
    let mut delegation_metadata_data = delegation_metadata_account.try_borrow_mut_data()?;
    (*delegation_metadata_data).copy_from_slice(delegation_metadata_serialized.as_slice());

    // Copy the data from the buffer into the original account
    if !buffer_account.data_is_empty() {
        let mut delegate_data = delegate_account.try_borrow_mut_data()?;
        let buffer_data = buffer_account.try_borrow_data()?;
        (*delegate_data).copy_from_slice(&buffer_data);
    }

    Ok(())
}
