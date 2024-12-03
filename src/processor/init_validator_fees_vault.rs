use solana_program::program_error::ProgramError;
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    {self},
};

use crate::consts::{ADMIN_PUBKEY, VALIDATOR_FEES_VAULT};
use crate::error::DlpError::Unauthorized;
use crate::processor::utils::loaders::{load_signer, load_uninitialized_pda};
use crate::processor::utils::pda::create_pda;

/// Process the initialization of the validator fees vault
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
    load_signer(payer)?;
    load_signer(admin)?;

    // Check if the admin is the correct one
    if !admin.key.eq(&ADMIN_PUBKEY) {
        return Err(Unauthorized.into());
    }

    let bump_fees_vault_record = load_uninitialized_pda(
        validator_fees_vault,
        &[VALIDATOR_FEES_VAULT, validator_identity.key.as_ref()],
        &crate::id(),
    )?;

    // Create the fees vault PDA
    create_pda(
        validator_fees_vault,
        &crate::id(),
        8,
        &[
            VALIDATOR_FEES_VAULT,
            validator_identity.key.as_ref(),
            &[bump_fees_vault_record],
        ],
        system_program,
        payer,
    )?;

    Ok(())
}
