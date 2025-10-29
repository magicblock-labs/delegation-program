use crate::args::SetFeesReceiverArgs;
use crate::error::DlpError::Unauthorized;
use crate::processor::utils::loaders::{
    load_initialized_protocol_fees_vault, load_program_upgrade_authority, load_signer,
};
use crate::processor::utils::pda::resize_pda;
use crate::state::discriminator::AccountDiscriminator;
use crate::state::FeesVault;
use borsh::BorshDeserialize;
use solana_program::msg;
use solana_program::program_error::ProgramError;
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey};

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
        let fees_vault_data = fees_vault_account.try_borrow_data()?;
        match FeesVault::try_from_bytes_with_discriminator(&fees_vault_data) {
            Ok(fees_vault) => (fees_vault, false),
            Err(_) => {
                // Migrating the account
                let mut data = vec![0; FeesVault::default().size_with_discriminator()];
                data[0..8].copy_from_slice(&AccountDiscriminator::FeesVault.to_bytes());
                let fees_vault = FeesVault::try_from_bytes_with_discriminator(&data)?;

                (fees_vault, true)
            }
        }
    };

    if migrated {
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
