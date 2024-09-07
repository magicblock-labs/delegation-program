use borsh::BorshDeserialize;
use solana_program::program::invoke;
use solana_program::program_error::ProgramError;
use solana_program::system_instruction::transfer;
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    pubkey::Pubkey,
    system_program, {self},
};
use std::mem::size_of;

use crate::consts::{EPHEMERAL_BALANCE, FEES_VAULT};
use crate::instruction::TopUpEphemeralArgs;
use crate::loaders::{load_initialized_pda, load_pda, load_signer};
use crate::state::EphemeralBalance;
use crate::utils::create_pda;
use crate::utils_account::{AccountDeserialize, Discriminator};

/// Process top up ephemeral balance
///
/// 1. Transfer lamports from payer to fees_vault PDA
/// 2. Create a user receipt account if it does not exist.
/// 3. Increase the receipt balance by the transferred amount
pub fn process_top_up_ephemeral(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    // Parse args.
    let args = TopUpEphemeralArgs::try_from_slice(data)?;

    // Load Accounts
    let [payer, ephemeral_balance_pda, fees_vault, system_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let seeds_ephemeral_balance_pda = [EPHEMERAL_BALANCE, &payer.key.to_bytes()];
    load_signer(payer)?;
    let bump_receipt_pda = load_pda(ephemeral_balance_pda, &seeds_ephemeral_balance_pda, true)?;
    load_initialized_pda(fees_vault, &[FEES_VAULT], &crate::id(), true)?;

    // Create the receipt account if it does not exist
    if ephemeral_balance_pda.owner.eq(&system_program::id()) {
        create_pda(
            ephemeral_balance_pda,
            &crate::id(),
            8 + size_of::<EphemeralBalance>(),
            &[
                EPHEMERAL_BALANCE,
                &payer.key.to_bytes(),
                &[bump_receipt_pda],
            ],
            system_program,
            payer,
        )?;
    }

    // Transfer lamports from payer to fees_vault PDA (with a system program call)
    let transfer_instruction = transfer(payer.key, fees_vault.key, args.amount);
    invoke(
        &transfer_instruction,
        &[payer.clone(), fees_vault.clone(), system_program.clone()],
    )?;

    msg!(
        "Amount {:?} transferred from {:?} to {:?}",
        args.amount,
        payer.key,
        fees_vault.key
    );

    // Add the lamports amount to the ephemeral balance
    let mut ephemeral_balance_data = ephemeral_balance_pda.try_borrow_mut_data()?;
    ephemeral_balance_data[0] = EphemeralBalance::discriminator() as u8;
    let ephemeral_balance = EphemeralBalance::try_from_bytes_mut(&mut ephemeral_balance_data)?;
    msg!(
        "Ephemeral balance before top up: {:?}",
        ephemeral_balance.lamports
    );
    ephemeral_balance.lamports += args.amount;
    msg!(
        "Ephemeral balance after top up: {:?}",
        ephemeral_balance.lamports
    );

    Ok(())
}
