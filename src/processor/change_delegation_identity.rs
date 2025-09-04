use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, msg, program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::processor::utils::loaders::{load_program_upgrade_authority, load_signer};
use crate::state::DelegationRecord;

/// Change the identity (authority) of the delegation record for a delegated account.
///
/// Only executable by the admin (upgrade authority) of this delegation program.
///
/// Accounts:
/// 0: [signer]   admin (upgrade authority of the delegation program)
/// 1: []         program data account for the delegation program (bpf upgradeable loader ProgramData PDA)
/// 2: [writable] delegation record PDA for the delegated account
/// 3: []         delegated account (used to derive the PDA)
///
/// Data:
/// - 32 bytes new identity (Pubkey)
pub fn process_change_delegation_identity(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let [admin, delegation_program_data, delegation_record_account, _delegated_account] = accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Expect exactly 32 bytes with the new identity
    if data.len() != 32 {
        msg!(
            "Invalid instruction data for ChangeDelegationIdentity, expected 32 bytes, got {}",
            data.len()
        );
        return Err(ProgramError::InvalidInstructionData);
    }

    // Validate signer
    load_signer(admin, "admin")?;

    // Validate that admin is the upgrade authority of this delegation program
    let expected_admin = load_program_upgrade_authority(&crate::ID, delegation_program_data)?
        .ok_or(ProgramError::InvalidAccountData)?;
    if admin.key.ne(&expected_admin) {
        msg!(
            "Unauthorized: expected admin {} but got {}",
            expected_admin,
            admin.key
        );
        return Err(crate::error::DlpError::Unauthorized.into());
    }

    // Deserialize, update authority, serialize back
    let mut data_ref = delegation_record_account.try_borrow_mut_data()?;
    let record = DelegationRecord::try_from_bytes_with_discriminator_mut(&mut data_ref)?;

    let mut new_identity_bytes = [0u8; 32];
    new_identity_bytes.copy_from_slice(data);
    record.authority = Pubkey::new_from_array(new_identity_bytes);

    // No need to rewrite discriminator/data as we modified in place (zero-copy structure)

    Ok(())
}
