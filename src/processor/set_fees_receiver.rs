use crate::args::SetFeesReceiverArgs;
use crate::error::DlpError::Unauthorized;
use crate::processor::utils::loaders::{
    load_program_config, load_program_upgrade_authority, load_signer,
};
use crate::processor::utils::pda::resize_pda;
use crate::state::ProgramConfig;
use borsh::BorshDeserialize;
use solana_program::msg;
use solana_program::program_error::ProgramError;
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey};

/// Process request to set the fees receiver
///
/// Accounts:
///
/// 1. `[signer, writable]`   admin account that can set the fees receiver
/// 2. `[writable]` program config PDA
/// 3. `[]` program
/// 4. `[]` system program
///
/// Requirements:
///
/// - program config is initialized
/// - admin is the protocol config admin
///
/// 1. Set the fees receiver in the protocol config
pub fn process_set_fees_receiver(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    // Load Accounts
    let [admin, program_config_account, program, system_program, delegation_program_data] =
        accounts
    else {
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

    // Check if the program config is initialized
    if !load_program_config(program_config_account, *program.key, true)? {
        return Err(ProgramError::UninitializedAccount);
    }

    // Migrate to the new account structure
    let (mut program_config, migrated) = {
        let program_config_data = program_config_account.try_borrow_data()?;
        match ProgramConfig::try_from_bytes_with_discriminator(&program_config_data) {
            Ok(program_config) => (program_config, false),
            Err(_) => {
                // Migrating the account
                let mut data = program_config_data.to_vec();
                data.extend(Pubkey::default().to_bytes());
                let program_config = ProgramConfig::try_from_bytes_with_discriminator(&data)?;

                (program_config, true)
            }
        }
    };

    if migrated {
        resize_pda(
            admin,
            program_config_account,
            system_program,
            program_config.size_with_discriminator(),
        )?;
    }

    let args = SetFeesReceiverArgs::try_from_slice(data)?;
    program_config.fees_receiver = args.fees_receiver;

    let mut program_config_data = program_config_account.try_borrow_mut_data()?;
    program_config.to_bytes_with_discriminator(&mut program_config_data.as_mut())?;

    Ok(())
}
