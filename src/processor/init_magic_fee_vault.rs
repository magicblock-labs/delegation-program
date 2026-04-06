use crate::{
    magic_fee_vault_seeds_from_validator,
    processor::utils::{
        loaders::{
            load_initialized_validator_fees_vault, load_program, load_signer,
            load_uninitialized_pda,
        },
        pda::create_pda,
    },
    solana_program::{
        account_info::AccountInfo, entrypoint::ProgramResult,
        program_error::ProgramError, pubkey::Pubkey, system_program,
    },
};

/// Process the initialization of the magic fee vault
///
/// Accounts:
///
/// 0; `[signer, writable]` payer
/// 1; `[signer]`           validator_identity
/// 2; `[]`                 validator_fees_vault
/// 3; `[writable]`         magic_fee_vault
/// 4; `[]`                 system_program
///
/// Requirements:
///
/// - validator must be a signer
/// - validator must have an initialized validator fees vault (i.e. be whitelisted)
/// - magic fee vault PDA must not be already initialized
///
/// 1. Verify validator is signer and whitelisted
/// 2. Create the magic fee vault PDA (not delegated)
pub fn process_init_magic_fee_vault(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    _data: &[u8],
) -> ProgramResult {
    let [payer, validator, validator_fees_vault, magic_fee_vault, system_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    load_signer(payer, "payer")?;
    load_signer(validator, "validator")?;
    load_program(system_program, system_program::id(), "system program")?;

    load_initialized_validator_fees_vault(
        validator,
        validator_fees_vault,
        false,
    )?;

    let magic_fee_vault_bump = load_uninitialized_pda(
        magic_fee_vault,
        magic_fee_vault_seeds_from_validator!(validator.key),
        &crate::id(),
        true,
        "magic fee vault",
    )?;

    create_pda(
        magic_fee_vault,
        &crate::id(),
        8,
        magic_fee_vault_seeds_from_validator!(validator.key),
        magic_fee_vault_bump,
        system_program,
        payer,
    )?;

    Ok(())
}
