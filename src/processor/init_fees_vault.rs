use solana_program::program_error::ProgramError;
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey, system_program,
};

use crate::fees_vault_seeds;
use crate::processor::utils::loaders::{load_program, load_signer, load_uninitialized_pda};
use crate::processor::utils::pda::create_pda;

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
    load_program(system_program, system_program::id())?;

    let bump_fees_vault =
        load_uninitialized_pda(fees_vault, fees_vault_seeds!(), &crate::id(), true)?;

    // Create the fees vault account
    create_pda(
        fees_vault,
        &crate::id(),
        8,
        fees_vault_seeds!(),
        bump_fees_vault,
        system_program,
        payer,
    )?;

    Ok(())
}
