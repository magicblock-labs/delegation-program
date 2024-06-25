use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{{self}, account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey};
use solana_program::program_error::ProgramError;
use crate::state::DelegationMetadata;

/// Called through CPI to allow the undelegation of a delegated account
///
pub fn process_allow_undelegate(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    _data: &[u8],
) -> ProgramResult {

    let [delegate_account, delegation_record, delegation_metadata, buffer] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Load delegated account metadata
    let mut metadata =
        DelegationMetadata::deserialize(&mut &**delegation_metadata.data.borrow())?;
    metadata.is_undelegatable = true;
    metadata.serialize(&mut &mut delegation_metadata.try_borrow_mut_data()?.as_mut())?;

    Ok(())
}
