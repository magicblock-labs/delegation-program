use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program_error::ProgramError,
    pubkey::Pubkey, system_program,
};

use crate::{
    error::DlpError::Unauthorized,
    fees_vault_seeds,
    processor::utils::{
        loaders::{
            load_program, load_program_upgrade_authority, load_signer, load_uninitialized_pda,
        },
        pda::create_pda,
    },
    state::FeesVault,
};

/// Initialize the global fees vault
///
/// Accounts:
/// 0: `[signer]`   the account paying for the transaction
/// 1: `[writable]` the fees vault PDA we are initializing
/// 2: `[]`         the delegation program data
/// 3: `[]`         the system program
///
/// Requirements:
///
/// - fees vault is uninitialized
///
/// NOTE: this operation is permisionless and can be done by anyone
///
/// Steps:
///
/// 1. Create the protocol fees vault PDA
pub fn process_init_protocol_fees_vault(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    _data: &[u8],
) -> ProgramResult {
    // Load Accounts
    let [payer, protocol_fees_vault, delegation_program_data, system_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    load_signer(payer, "payer")?;
    load_program(system_program, system_program::id(), "system program")?;

    let bump_fees_vault = load_uninitialized_pda(
        protocol_fees_vault,
        fees_vault_seeds!(),
        &crate::id(),
        true,
        "fees vault",
    )?;

    // Check if the admin is the correct one
    let admin_pubkey =
        load_program_upgrade_authority(&crate::ID, delegation_program_data)?.ok_or(Unauthorized)?;

    let fees_vault = FeesVault {
        fees_receiver: admin_pubkey,
    };

    // Create the fees vault account
    create_pda(
        protocol_fees_vault,
        &crate::id(),
        fees_vault.size_with_discriminator(),
        fees_vault_seeds!(),
        bump_fees_vault,
        system_program,
        payer,
    )?;

    // Write the fees vault data
    let mut fees_vault_data = protocol_fees_vault.try_borrow_mut_data()?;
    fees_vault.to_bytes_with_discriminator(&mut fees_vault_data.as_mut())?;

    Ok(())
}
