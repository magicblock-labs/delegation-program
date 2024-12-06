use solana_program::program_error::ProgramError;
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    {self},
};

use crate::consts::{BUFFER, DELEGATION_METADATA, DELEGATION_RECORD};
use crate::processor::utils::loaders::{load_initialized_pda, load_owned_pda, load_signer};
use crate::state::{DelegationMetadata, DelegationRecord};

/// Called through CPI to allow the undelegation of a delegated account
///
pub fn process_allow_undelegate(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    _data: &[u8],
) -> ProgramResult {
    let [delegated_account, delegation_record_account, delegation_metadata_account, buffer] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Check the buffer PDA is a signer, to ensure this instruction is called from CPI
    load_signer(buffer)?;

    // Check that the account is owned by the delegation program
    load_owned_pda(delegated_account, &crate::id())?;

    // Check delegation record
    load_initialized_pda(
        delegation_record_account,
        &[DELEGATION_RECORD, &delegated_account.key.to_bytes()],
        &crate::id(),
        false,
    )?;

    // Check delegation metadata
    load_initialized_pda(
        delegation_metadata_account,
        &[DELEGATION_METADATA, &delegated_account.key.to_bytes()],
        &crate::id(),
        true,
    )?;

    // Read delegation record
    let delegation_record_data = delegation_record_account.try_borrow_data()?;
    let delegation_record =
        DelegationRecord::try_from_bytes_with_discriminant(&delegation_record_data)?;

    // Check that the buffer PDA is initialized and derived correctly from the PDA
    let buffer_pda = Pubkey::find_program_address(
        &[BUFFER, &delegated_account.key.to_bytes()],
        &delegation_record.owner,
    );
    if buffer.key.ne(&buffer_pda.0) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Load and update delegated account metadata
    let mut delegation_metadata_data = delegation_metadata_account.try_borrow_mut_data()?;
    let mut delegation_metadata =
        DelegationMetadata::try_from_bytes_with_discriminant(&delegation_metadata_data)?;
    delegation_metadata.is_undelegatable = true;
    delegation_metadata.to_bytes_with_discriminant(&mut delegation_metadata_data.as_mut())?;

    Ok(())
}
