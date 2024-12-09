use crate::ephemeral_balance_seeds_from_payer;
use crate::processor::utils::loaders::{load_initialized_pda, load_signer};
use crate::processor::utils::pda::close_pda;
use solana_program::program_error::ProgramError;
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey};

pub fn process_close_ephemeral_balance(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let index = *data.first().ok_or(ProgramError::InvalidInstructionData)?;

    // Load Accounts
    let [payer, ephemeral_balance_pda] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    load_signer(payer)?;

    load_initialized_pda(
        ephemeral_balance_pda,
        ephemeral_balance_seeds_from_payer!(payer.key, index),
        &crate::id(),
        true,
    )?;

    close_pda(ephemeral_balance_pda, payer)?;

    Ok(())
}
