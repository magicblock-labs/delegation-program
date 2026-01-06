use pinocchio::{
    account_info::AccountInfo, instruction::Signer, program_error::ProgramError, pubkey::Pubkey,
    seeds, ProgramResult,
};

use crate::error::DlpError;
use crate::processor::fast::utils::requires::require_authorization;
use crate::processor::fast::utils::{
    pda::{close_pda, create_pda},
    requires::{
        require_initialized_delegation_metadata, require_initialized_delegation_record,
        require_owned_pda, require_signer, require_uninitialized_pda, UndelegateBufferCtx,
    },
};
use crate::state::{DelegationMetadata, DelegationRecord};
use crate::{pda, require_eq_keys};

use super::{process_undelegation_with_cpi, to_pinocchio_program_error};

/// Admin-only undelegation for confined accounts (authority == system program)
/// Accounts:
///  0: [signer]    admin (must equal program upgrade authority)
///  1: [writable]  delegated account (owned by delegation program)
///  2: []          owner program of the delegated account
///  3: [writable]  undelegate buffer PDA (delegation program)
///  4: [writable]  delegation record PDA
///  5: [writable]  delegation metadata PDA
///  6: []          system program
///  7: []          delegation program data (UpgradeableLoader ProgramData of this program)
pub fn process_undelegate_confined_account(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    _data: &[u8],
) -> ProgramResult {
    let [admin, delegated_account, owner_program, undelegate_buffer_account, delegation_record_account, delegation_metadata_account, system_program, delegation_program_data] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Admin must sign
    require_signer(admin, "admin")?;

    // Verify admin is the program upgrade authority.
    require_authorization(delegation_program_data, admin)?;

    // Basic checks
    require_owned_pda(delegated_account, &crate::fast::ID, "delegated account")?;
    require_initialized_delegation_record(delegated_account, delegation_record_account, true)?;
    require_initialized_delegation_metadata(delegated_account, delegation_metadata_account, true)?;

    // Load delegation record
    let delegation_record_data = delegation_record_account.try_borrow_data()?;
    let delegation_record =
        DelegationRecord::try_from_bytes_with_discriminator(&delegation_record_data)
            .map_err(to_pinocchio_program_error)?;

    let delegation_metadata_data = delegation_metadata_account.try_borrow_data()?;
    let delegation_metadata =
        DelegationMetadata::try_from_bytes_with_discriminator(&delegation_metadata_data)
            .map_err(to_pinocchio_program_error)?;

    // Confined account: authority must be system program
    require_eq_keys!(
        &delegation_record.authority.to_bytes(),
        &pinocchio_system::ID,
        DlpError::InvalidAuthority
    );

    // Owner must match the one stored in the record
    require_eq_keys!(
        &delegation_record.owner.to_bytes(),
        owner_program.key(),
        ProgramError::InvalidAccountOwner
    );

    // If the delegated account has no data, simply assign back to the original owner and clean up PDAs
    if delegated_account.data_is_empty() {
        unsafe {
            delegated_account.assign(owner_program.key());
        }
        // Close PDAs to the admin
        close_pda(delegation_record_account, admin)?;
        close_pda(delegation_metadata_account, admin)?;
        return Ok(());
    }

    // Initialize undelegation buffer PDA and copy state
    let undelegate_buffer_bump: u8 = require_uninitialized_pda(
        undelegate_buffer_account,
        &[pda::UNDELEGATE_BUFFER_TAG, delegated_account.key()],
        &crate::fast::ID,
        true,
        UndelegateBufferCtx,
    )?;

    create_pda(
        undelegate_buffer_account,
        &crate::fast::ID,
        delegated_account.data_len(),
        &[Signer::from(&seeds!(
            pda::UNDELEGATE_BUFFER_TAG,
            delegated_account.key(),
            &[undelegate_buffer_bump]
        ))],
        admin,
    )?;

    (*undelegate_buffer_account.try_borrow_mut_data()?)
        .copy_from_slice(&delegated_account.try_borrow_data()?);

    // Reuse the exact same CPI helper as process_undelegate
    process_undelegation_with_cpi(
        admin,
        delegated_account,
        owner_program,
        undelegate_buffer_account,
        &[Signer::from(&seeds!(
            pda::UNDELEGATE_BUFFER_TAG,
            delegated_account.key(),
            &[undelegate_buffer_bump]
        ))],
        delegation_metadata,
        system_program,
    )?;

    // Dropping delegation references
    drop(delegation_record_data);
    drop(delegation_metadata_data);

    // Close buffer and delegation PDAs
    close_pda(undelegate_buffer_account, admin)?;
    close_pda(delegation_record_account, admin)?;
    close_pda(delegation_metadata_account, admin)?;

    Ok(())
}
