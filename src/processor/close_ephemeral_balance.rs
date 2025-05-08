use crate::ephemeral_balance_seeds_from_payer;
use crate::processor::utils::loaders::{load_pda, load_signer};
use solana_program::msg;
use solana_program::program::invoke_signed;
use solana_program::program_error::ProgramError;
use solana_program::system_instruction::transfer;
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey, system_program,
};

/// Process the closing of an ephemeral balance account
///
/// Accounts:
///
/// 0: `[signer]` payer to pay for the transaction and receive the refund
/// 1: `[writable]` ephemeral balance account we are closing
/// 2: `[]` the system program
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
    let [payer, ephemeral_balance_account, system_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    load_signer(payer, "payer")?;

    let ephemeral_balance_seeds: &[&[u8]] = ephemeral_balance_seeds_from_payer!(payer.key, index);
    let ephemeral_balance_bump = load_pda(
        ephemeral_balance_account,
        ephemeral_balance_seeds,
        &crate::id(),
        true,
        "ephemeral balance",
    )?;
    if ephemeral_balance_account.owner != &system_program::id() {
        msg!(
            "ephemeral balance expected to be owned by system program: {}",
            system_program::id()
        );
        return Err(ProgramError::InvalidAccountOwner);
    }

    let amount = ephemeral_balance_account.lamports();
    if amount == 0 {
        return Ok(());
    }

    let ephemeral_balance_bump_slice: &[u8] = &[ephemeral_balance_bump];
    let ephemeral_balance_signer_seeds =
        [ephemeral_balance_seeds, &[ephemeral_balance_bump_slice]].concat();
    invoke_signed(
        &transfer(ephemeral_balance_account.key, payer.key, amount),
        &[
            ephemeral_balance_account.clone(),
            payer.clone(),
            system_program.clone(),
        ],
        &[&ephemeral_balance_signer_seeds],
    )?;

    Ok(())
}
