use crate::ephemeral_balance_seeds_from_payer;
use crate::processor::utils::loaders::{load_initialized_pda, load_signer};
use crate::processor::utils::pda::close_pda;
use solana_program::program_error::ProgramError;
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey};

/// Process the closing of an ephemeral balance account
///
/// Accounts:
///
/// 0: `[signer]` payer to pay for the transaction and receive the refund
/// 1: `[writable]` ephemeral balance account we are closing
///
/// Requirements:
///
/// - ephemeral balance account is initialized
///
/// Steps:
///
/// 1. Closes the ephemeral balance account and refunds the payer with the
///    escrowed lamports
pub fn process_close_ephemeral_balance(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let index = *data.first().ok_or(ProgramError::InvalidInstructionData)?;

    // Load Accounts
    let [payer, ephemeral_balance_account] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    load_signer(payer, "payer")?;

    load_initialized_pda(
        ephemeral_balance_account,
        ephemeral_balance_seeds_from_payer!(payer.key, index),
        &crate::id(),
        true,
        "ephemeral balance",
    )?;

    close_pda(ephemeral_balance_account, payer)?;

    Ok(())
}
