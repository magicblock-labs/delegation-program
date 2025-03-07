use solana_program::program_error::ProgramError;
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey, system_program,
};

use crate::fees_vault_seeds;
use crate::processor::utils::loaders::{load_program, load_signer, load_uninitialized_pda};
use crate::processor::utils::pda::create_pda;

/// Initialize the global fees vault
///
/// Accounts:
/// - `[signer]` The account paying for the transaction
/// - `[writable]` The fees vault PDA we are initializing
/// - `[]` The system program
///
/// Requirements:
///
/// - fees vault is uninitialized
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
    let [payer, protocol_fees_vault, system_program] = accounts else {
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

    // Create the fees vault account
    create_pda(
        protocol_fees_vault,
        &crate::id(),
        8,
        fees_vault_seeds!(),
        bump_fees_vault,
        system_program,
        payer,
    )?;

    Ok(())
}
