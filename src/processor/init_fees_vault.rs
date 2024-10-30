use solana_program::program_error::ProgramError;
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    {self},
};

use crate::consts::FEES_VAULT;
use crate::loaders::{load_signer, load_uninitialized_pda};
use crate::utils::create_pda;

/// Initialize the global fees vault
///
/// 1. Create the fees vault PDA
pub fn process_init_fees_vault(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    _data: &[u8],
) -> ProgramResult {
    // Load Accounts
    let [payer, fees_vault, system_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    load_signer(payer)?;
    let bump_fees_vault = load_uninitialized_pda(fees_vault, &[FEES_VAULT], &crate::id())?;

    // Crete the receipt account if it does not exist
    create_pda(
        fees_vault,
        &crate::id(),
        8,
        &[FEES_VAULT, &[bump_fees_vault]],
        system_program,
        payer,
    )?;

    Ok(())
}
