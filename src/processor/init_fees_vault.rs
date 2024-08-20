use solana_program::{{self}, account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey};
use solana_program::program_error::ProgramError;

use crate::consts::FEES_VAULT;
use crate::loaders::{load_signer, load_uninitialized_pda};
use crate::utils::create_pda;

/// Process top up ephemeral
///
/// 1. Transfer lamports from payer to fees_vault PDA
/// 2. Create a user receipt account if it does not exist. Increase the receipt balance by the transferred amount
pub fn process_init_fees_vault(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    _data: &[u8],
) -> ProgramResult {

    // Load Accounts
    let [payer, fees_vault,  system_program] =
        accounts
    else {
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
        &system_program,
        payer,
    )?;

    Ok(())
}
