use borsh::BorshDeserialize;
use pinocchio::instruction::{Seed, Signer};
use pinocchio::pubkey::{self, pubkey_eq};
use pinocchio::sysvars::clock::Clock;
use pinocchio::sysvars::Sysvar;
use pinocchio::{
    account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey, ProgramResult,
};
use pinocchio_log::log;

use crate::args::DelegateArgs;
use crate::consts::DEFAULT_VALIDATOR_IDENTITY;
use crate::error::DlpError;
use crate::pda;
use crate::processor::fast::to_pinocchio_program_error;
use crate::processor::fast::utils::{pda::create_pda, requires::require_uninitialized_pda};
use crate::processor::utils::curve::is_on_curve_fast;
use crate::state::{DelegationMetadata, DelegationRecord};

use super::utils::requires::{require_owned_pda, require_pda, require_signer};

pub fn process_delegate(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let [payer, delegated_account, owner_program, delegate_buffer_account, delegation_record_account, delegation_metadata_account, _system_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    require_owned_pda(delegated_account, &crate::fast::ID, "delegated account")?;

    // we do not need system_program anywhere
    // require_program(system_program, &pinocchio_system::ID, "system program")?;

    // Check that payer and delegated_account are signers, this ensures the instruction is being called from CPI
    require_signer(payer, "payer")?;
    require_signer(delegated_account, "delegated account")?;

    // Check that the buffer PDA is initialized and derived correctly from the PDA
    require_pda(
        delegate_buffer_account,
        &[pda::DELEGATE_BUFFER_TAG, delegated_account.key()],
        owner_program.key(),
        true,
        "delegate buffer",
    )?;

    // Check that the delegation record PDA is uninitialized
    // TODO (snawaz): This check could be safely avoided, as create_pda would anyway fail.
    // Could save considerable CU, especially in the v2 version where we will pass the bumps
    let delegation_record_bump = require_uninitialized_pda(
        delegation_record_account,
        &[pda::DELEGATION_RECORD_TAG, delegated_account.key()],
        &crate::fast::ID,
        true,
        "delegation record",
    )?;

    // Check that the delegation metadata PDA is uninitialized
    // TODO (snawaz): This check could be safely avoided, as create_pda would anyway fail.
    // Could save considerable CU, especially in the v2 version where we will pass the bumps
    let delegation_metadata_bump = require_uninitialized_pda(
        delegation_metadata_account,
        &[pda::DELEGATION_METADATA_TAG, delegated_account.key()],
        &crate::fast::ID,
        true,
        "delegation metadata",
    )?;

    let args =
        DelegateArgs::try_from_slice(data).map_err(|_| ProgramError::InvalidInstructionData)?;

    // Validate seeds if the delegate account is not on curve, i.e. is a PDA
    // If the owner is the system program, we check if the account is derived from the delegation program,
    // allowing delegation of escrow accounts
    if !is_on_curve_fast(delegated_account.key()) {
        let program_id = if pubkey_eq(owner_program.key(), &pinocchio_system::ID) {
            &crate::fast::ID
        } else {
            owner_program.key()
        };
        let seeds_to_validate: &[&[u8]] = match args.seeds.len() {
            1 => &[&args.seeds[0]],
            2 => &[&args.seeds[0], &args.seeds[1]],
            3 => &[&args.seeds[0], &args.seeds[1], &args.seeds[2]],
            4 => &[
                &args.seeds[0],
                &args.seeds[1],
                &args.seeds[2],
                &args.seeds[3],
            ],
            _ => return Err(DlpError::TooManySeeds.into()),
        };
        let derived_pda = pubkey::find_program_address(seeds_to_validate, program_id).0;

        if !pubkey_eq(&derived_pda, delegated_account.key()) {
            log!("Expected delegated PDA to be: ");
            pubkey::log(&derived_pda);
            log!("but got: ");
            pubkey::log(delegated_account.key());
            return Err(ProgramError::InvalidSeeds);
        }
    }

    create_pda(
        delegation_record_account,
        &crate::fast::ID,
        DelegationRecord::size_with_discriminator(),
        &[Signer::from(&[
            Seed::from(pda::DELEGATION_RECORD_TAG),
            Seed::from(delegated_account.key()),
            Seed::from(&[delegation_record_bump]),
        ])],
        payer,
    )?;

    // Initialize the delegation record
    let delegation_record = DelegationRecord {
        owner: (*owner_program.key()).into(),
        authority: args.validator.unwrap_or(DEFAULT_VALIDATOR_IDENTITY),
        commit_frequency_ms: args.commit_frequency_ms as u64,
        delegation_slot: Clock::get()?.slot,
        lamports: delegated_account.lamports(),
    };

    let mut delegation_record_data = delegation_record_account.try_borrow_mut_data()?;
    delegation_record
        .to_bytes_with_discriminator(&mut delegation_record_data)
        .map_err(to_pinocchio_program_error)?;

    let delegation_metadata = DelegationMetadata {
        seeds: args.seeds,
        last_update_nonce: 0,
        is_undelegatable: false,
        rent_payer: (*payer.key()).into(),
    };

    // Initialize the delegation metadata PDA
    create_pda(
        delegation_metadata_account,
        &crate::fast::ID,
        delegation_metadata.serialized_size(),
        &[Signer::from(&[
            Seed::from(pda::DELEGATION_METADATA_TAG),
            Seed::from(delegated_account.key()),
            Seed::from(&[delegation_metadata_bump]),
        ])],
        payer,
    )?;

    // Copy the seeds to the delegated metadata PDA
    let mut delegation_metadata_data = delegation_metadata_account.try_borrow_mut_data()?;
    delegation_metadata
        .to_bytes_with_discriminator(&mut delegation_metadata_data.as_mut())
        .map_err(to_pinocchio_program_error)?;

    // Copy the data from the buffer into the original account
    if !delegate_buffer_account.data_is_empty() {
        let mut delegated_data = delegated_account.try_borrow_mut_data()?;
        let delegate_buffer_data = delegate_buffer_account.try_borrow_data()?;
        (*delegated_data).copy_from_slice(&delegate_buffer_data);
    }

    Ok(())
}
