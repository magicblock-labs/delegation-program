use borsh::BorshDeserialize;
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, msg, program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::{
    args::SetFeesReceiverArgs,
    error::DlpError::Unauthorized,
    processor::utils::{
        loaders::{
            load_initialized_protocol_fees_vault, load_program, load_program_upgrade_authority,
            load_signer,
        },
        pda::resize_pda,
    },
    state::FeesVault,
};

/// Process request to set the fees receiver
///
/// Accounts:
///
/// 1. `[signer, writable]`   admin account that can set the fees receiver
/// 2. `[writable]` fees vault PDA
/// 3. `[]` delegation program data
/// 4. `[]` system program
///
/// Requirements:
///
/// - admin is the program upgrade authority
/// - fees vault is initialized
///
/// 1. Set the fees receiver in the [FeesVault] account
pub fn process_set_fees_receiver(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    // Load Accounts
    let [admin, fees_vault_account, delegation_program_data, system_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Check if the admin is signer
    load_signer(admin, "admin")?;

    // Check if the admin is the correct one
    let admin_pubkey =
        load_program_upgrade_authority(&crate::ID, delegation_program_data)?.ok_or(Unauthorized)?;
    if !admin.key.eq(&admin_pubkey) {
        msg!(
            "Expected admin pubkey: {} but got {}",
            admin_pubkey,
            admin.key
        );
        return Err(Unauthorized.into());
    }

    // Check if the fees vault is initialized
    load_initialized_protocol_fees_vault(fees_vault_account, true)?;

    // Migrate to the new fees vault structure
    let (mut fees_vault, migrated) = {
        let data = fees_vault_account.try_borrow_data()?;
        match FeesVault::try_from_bytes_with_discriminator(&data) {
            Ok(fv) => (fv, false),
            Err(_) => (FeesVault::default(), true),
        }
    };

    if migrated {
        load_program(
            system_program,
            solana_program::system_program::ID,
            "system program",
        )?;
        resize_pda(
            admin,
            fees_vault_account,
            system_program,
            fees_vault.size_with_discriminator(),
        )?;
    }

    let args = SetFeesReceiverArgs::try_from_slice(data)?;
    fees_vault.fees_receiver = args.fees_receiver;

    let mut fees_vault_data = fees_vault_account.try_borrow_mut_data()?;
    fees_vault.to_bytes_with_discriminator(&mut fees_vault_data.as_mut())?;

    Ok(())
}
