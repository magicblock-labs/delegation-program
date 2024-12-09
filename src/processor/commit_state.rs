use std::mem::size_of;

use crate::args::CommitStateArgs;
use crate::error::DlpError;
use crate::processor::utils::loaders::{
    load_initialized_delegation_metadata, load_initialized_delegation_record,
    load_initialized_validator_fees_vault, load_owned_pda, load_program, load_program_config,
    load_signer, load_uninitialized_pda,
};
use crate::processor::utils::pda::create_pda;
use crate::processor::utils::verify::verify_state;
use crate::state::account::{AccountDeserialize, AccountWithDiscriminator};
use crate::state::{CommitRecord, DelegationMetadata, DelegationRecord, ProgramConfig};
use crate::{
    commit_record_seeds_from_delegated_account, commit_state_seeds_from_delegated_account,
};
use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::program::invoke;
use solana_program::program_error::ProgramError;
use solana_program::system_instruction::transfer;
use solana_program::system_program;
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    pubkey::Pubkey,
    {self},
};

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
    let args = CommitStateArgs::try_from_slice(data)?;
    let delegated_data: &[u8] = args.data.as_ref();

    let [validator, delegated_account, commit_state_account, commit_record_account, delegation_record_account, delegation_metadata_account, validator_fees_vault, program_config_account, system_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Check that the origin account is delegated
    load_owned_pda(delegated_account, &crate::id())?;
    load_signer(validator)?;
    load_initialized_delegation_record(delegated_account, delegation_record_account, false)?;
    load_initialized_delegation_metadata(delegated_account, delegation_metadata_account, true)?;
    load_initialized_validator_fees_vault(validator, validator_fees_vault, false)?;
    load_program(system_program, system_program::id())?;

    // Load delegation record
    let mut delegation_record_data = delegation_record_account.try_borrow_data()?;
    let delegation_record = DelegationRecord::try_from_bytes(&mut delegation_record_data)?;

    // Load the program configuration and validate it, if any
    let has_program_config = load_program_config(program_config_account, delegation_record.owner)?;
    if has_program_config {
        let program_config_data = program_config_account.try_borrow_data()?;
        let program_config = ProgramConfig::try_from_slice(&program_config_data)?;
        msg!("Program Config: {:?}", program_config);
        validate_program_config(program_config, validator.key)?;
    }

    // Load the uninitialized PDAs
    let commit_state_bump = load_uninitialized_pda(
        commit_state_account,
        commit_state_seeds_from_delegated_account!(delegated_account.key),
        &crate::id(),
    )?;
    let commit_record_bump = load_uninitialized_pda(
        commit_record_account,
        commit_record_seeds_from_delegated_account!(delegated_account.key),
        &crate::id(),
    )?;

    // Initialize the PDA containing the new committed state
    create_pda(
        commit_state_account,
        &crate::id(),
        delegated_data.len(),
        commit_state_seeds_from_delegated_account!(delegated_account.key),
        commit_state_bump,
        system_program,
        validator,
    )?;

    // Initialize the PDA containing the record of the committed state
    create_pda(
        commit_record_account,
        &crate::id(),
        8 + size_of::<CommitRecord>(),
        commit_record_seeds_from_delegated_account!(delegated_account.key),
        commit_record_bump,
        system_program,
        validator,
    )?;

    // What to do in this case?
    if delegated_account.lamports() < delegation_record.lamports {
        return Err(DlpError::InvalidDelegatedState.into());
    }

    // If committed lamports are more than the previous lamports balance, deposit the difference in the commitment account
    // If committed lamports are less than the previous lamports balance, we have collateral to settle the balance at state finalization
    if args.lamports > delegation_record.lamports {
        let difference = args.lamports - delegation_record.lamports;
        let transfer_instruction = transfer(validator.key, commit_state_account.key, difference);
        invoke(
            &transfer_instruction,
            &[
                validator.clone(),
                commit_state_account.clone(),
                system_program.clone(),
            ],
        )?;
    }

    // Initialize the commit record
    let mut commit_record_data = commit_record_account.try_borrow_mut_data()?;
    commit_record_data[0] = CommitRecord::discriminator() as u8;
    let commit_record = CommitRecord::try_from_bytes_mut(&mut commit_record_data)?;
    commit_record.identity = *validator.key;
    commit_record.account = *delegated_account.key;
    commit_record.slot = args.slot;
    commit_record.lamports = args.lamports;

    // Update delegation metadata undelegation flag
    let mut delegation_metadata_data = delegation_metadata_account.try_borrow_mut_data()?;
    let mut delegation_metadata = DelegationMetadata::try_from_slice(&delegation_metadata_data)?;
    delegation_metadata.is_undelegatable = args.allow_undelegation;
    delegation_metadata.serialize(&mut &mut delegation_metadata_data.as_mut())?;

    // Copy the new state to the initialized PDA
    let mut commit_state_data = commit_state_account.try_borrow_mut_data()?;
    (*commit_state_data).copy_from_slice(delegated_data);

    verify_commitment(
        validator,
        delegation_record,
        commit_record,
        commit_state_account,
    )?;

    Ok(())
}

/// If there exists a validators whitelist for the delegated account program owner, check that the validator is whitelisted for it
fn validate_program_config(
    program_config: ProgramConfig,
    validator: &Pubkey,
) -> Result<(), ProgramError> {
    if !program_config.approved_validators.is_empty()
        && !program_config.approved_validators.contains(validator)
    {
        return Err(DlpError::InvalidWhitelistProgramConfig.into());
    }
    msg!("Valid config");
    Ok(())
}

/// Verify the committed state
fn verify_commitment(
    authority: &AccountInfo,
    delegation_record: &DelegationRecord,
    commit_record: &CommitRecord,
    commit_state_account: &AccountInfo,
) -> ProgramResult {
    // TODO - is there something special to do here?
    verify_state(
        authority,
        delegation_record,
        commit_record,
        commit_state_account,
    )
}
