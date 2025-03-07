use solana_program::msg;
use solana_program::program_error::ProgramError;
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey, system_program,
};

use crate::consts::ADMIN_PUBKEY;
use crate::error::DlpError::Unauthorized;
use crate::processor::utils::loaders::{load_program, load_signer, load_uninitialized_pda};
use crate::processor::utils::pda::create_pda;
use crate::validator_fees_vault_seeds_from_validator;

/// Process the initialization of the validator fees vault
///
/// Accounts:
///
/// 0; `[signer]` payer
/// TODO: @@@ why does this need to be a signer?
/// 1; `[signer]` admin that controls the vault
/// 2; `[]` validator_identity
/// 3; `[]` validator_fees_vault_pda
/// 4; `[]` system_program
///
/// Requirements:
///
/// - validator admin is whitelisted
/// - validator fees vault is not initialized
///
/// 1. Create the validator fees vault PDA
/// 2. Currently, the existence of the validator fees vault also act as a flag to indicate that the validator is whitelisted (only the admin can create the vault)
pub fn process_init_validator_fees_vault(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    _data: &[u8],
) -> ProgramResult {
    // Load Accounts
    let [payer, admin, validator_identity, validator_fees_vault, system_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Check if the payer and admin are signers
    load_signer(payer, "payer")?;
    load_signer(admin, "admin")?;
    load_program(system_program, system_program::id(), "system program")?;

    // Check if the admin is the correct one
    if !admin.key.eq(&ADMIN_PUBKEY) {
        msg!(
            "Expected admin pubkey: {} but got {}",
            ADMIN_PUBKEY,
            admin.key
        );
        return Err(Unauthorized.into());
    }

    let validator_fees_vault_bump = load_uninitialized_pda(
        validator_fees_vault,
        validator_fees_vault_seeds_from_validator!(validator_identity.key),
        &crate::id(),
        true,
        "validator fees vault",
    )?;

    // Create the fees vault PDA
    create_pda(
        validator_fees_vault,
        &crate::id(),
        8,
        validator_fees_vault_seeds_from_validator!(validator_identity.key),
        validator_fees_vault_bump,
        system_program,
        payer,
    )?;

    Ok(())
}
