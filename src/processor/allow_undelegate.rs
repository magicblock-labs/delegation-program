use borsh::BorshSerialize;
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
    let [delegated_account, delegation_record, delegation_metadata, buffer] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Check the buffer PDA is a signer, to ensure this instruction is called from CPI
    load_signer(buffer)?;

    // Check that the account is owned by the delegation program
    load_owned_pda(delegated_account, &crate::id())?;

    // Check delegation record
    load_initialized_pda(
        delegation_record,
        &[DELEGATION_RECORD, &delegated_account.key.to_bytes()],
        &crate::id(),
        false,
    )?;

    // Check delegation metadata
    load_initialized_pda(
        delegation_metadata,
        &[DELEGATION_METADATA, &delegated_account.key.to_bytes()],
        &crate::id(),
        true,
    )?;

    let delegation_data = delegation_record.try_borrow_data()?;
    let delegation = DelegationRecord::try_from_bytes_with_discriminant(&delegation_data)?;

    // Check that the buffer PDA is initialized and derived correctly from the PDA
    let pda = Pubkey::find_program_address(
        &[BUFFER, &delegated_account.key.to_bytes()],
        &delegation.owner,
    );
    if buffer.key.ne(&pda.0) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Load delegated account metadata
    let mut metadata = DelegationMetadata::try_from_bytes_with_discriminant(
        &delegation_metadata.try_borrow_data()?,
    )?;
    metadata.is_undelegatable = true;
    metadata.serialize(&mut &mut delegation_metadata.try_borrow_mut_data()?.as_mut())?;

    Ok(())
}
