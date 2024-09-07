use borsh::BorshDeserialize;
use solana_program::program_error::ProgramError;
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    {self},
};

use crate::consts::{EPHEMERAL_BALANCE, FEES_VAULT};
use crate::instruction::WithdrawArgs;
use crate::loaders::{load_initialized_pda, load_signer};
use crate::state::EphemeralBalance;
use crate::utils_account::AccountDeserialize;

/// Process withdraw from ephemeral balance
///
/// 1. Transfer lamports from payer to fees_vault PDA
/// 2. Create a user receipt account if it does not exist. Increase the receipt balance by the transferred amount
pub fn process_withdraw(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    // Parse args.
    let args = WithdrawArgs::try_from_slice(data)?;

    // Load Accounts
    let [payer, ephemeral_balance_pda, fees_vault] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    load_signer(payer)?;
    load_initialized_pda(
        ephemeral_balance_pda,
        &[EPHEMERAL_BALANCE, &payer.key.to_bytes()],
        &crate::id(),
        true,
    )?;
    load_initialized_pda(fees_vault, &[FEES_VAULT], &crate::id(), true)?;

    // Parse the ephemeral balance account
    let mut ephemeral_balance_data = ephemeral_balance_pda.data.borrow_mut();
    let ephemeral_balance = EphemeralBalance::try_from_bytes_mut(&mut ephemeral_balance_data)?;

    let amount = args.amount.unwrap_or(ephemeral_balance.lamports);

    // Ensure the ephemeral balance has enough lamports
    if ephemeral_balance.lamports < amount {
        return Err(ProgramError::InsufficientFunds);
    }

    // Subtract the amount from the ephemeral balance
    ephemeral_balance.lamports -= amount;

    // Transfer lamports from fees_vault PDA to payer
    **fees_vault.try_borrow_mut_lamports()? = fees_vault
        .lamports()
        .checked_sub(amount)
        .ok_or(ProgramError::InsufficientFunds)?;

    **payer.try_borrow_mut_lamports()? = payer
        .lamports()
        .checked_add(amount)
        .ok_or(ProgramError::InvalidArgument)?;

    Ok(())
}
