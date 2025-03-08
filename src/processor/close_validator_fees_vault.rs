use solana_program::msg;
use solana_program::program_error::ProgramError;
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey};

use crate::consts::ADMIN_PUBKEY;
use crate::error::DlpError::Unauthorized;
use crate::processor::utils::loaders::{load_initialized_pda, load_signer};
use crate::processor::utils::pda::close_pda;
use crate::validator_fees_vault_seeds_from_validator;

/// Process the close of the validator fees vault
///
/// Accounts:
///
/// 0; `[signer]` payer
/// 1; `[signer]` admin that controls the vault
/// 2; `[]`       validator_identity
/// 3; `[]`       validator_fees_vault_pda
///
/// Requirements:
///
/// - validator admin need to be signer since the existence of the validator fees vault
///   is used as proof later that the validator is whitelisted
/// - validator fees vault is closed
///
/// 1. Close the validator fees vault PDA
pub fn process_close_validator_fees_vault(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    _data: &[u8],
) -> ProgramResult {
    // Load Accounts
    let [payer, admin, validator_identity, validator_fees_vault] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Check if the payer and admin are signers
    load_signer(payer, "payer")?;
    load_signer(admin, "admin")?;

    // Check if the admin is the correct one
    if !admin.key.eq(&ADMIN_PUBKEY) {
        msg!(
            "Expected admin pubkey: {} but got {}",
            ADMIN_PUBKEY,
            admin.key
        );
        return Err(Unauthorized.into());
    }

    load_initialized_pda(
        validator_fees_vault,
        validator_fees_vault_seeds_from_validator!(validator_identity.key),
        &crate::id(),
        true,
        "validator fees vault",
    )?;

    // Close the fees vault PDA
    close_pda(validator_fees_vault, validator_identity)?;

    Ok(())
}
