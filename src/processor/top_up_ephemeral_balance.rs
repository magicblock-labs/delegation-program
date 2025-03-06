use crate::args::TopUpEphemeralBalanceArgs;
use crate::ephemeral_balance_seeds_from_payer;
use crate::processor::utils::loaders::{load_pda, load_program, load_signer};
use crate::processor::utils::pda::create_pda;
use borsh::BorshDeserialize;
use solana_program::program::invoke;
use solana_program::program_error::ProgramError;
use solana_program::system_instruction::transfer;
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey, system_program,
};

pub fn process_top_up_ephemeral_balance(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    // Parse args.
    let args = TopUpEphemeralBalanceArgs::try_from_slice(data)?;

    // Load Accounts
    let [payer, pubkey, ephemeral_balance_account, system_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    load_signer(payer, "payer")?;
    load_program(system_program, system_program::id(), "system program")?;

    let bump_ephemeral_balance = load_pda(
        ephemeral_balance_account,
        ephemeral_balance_seeds_from_payer!(pubkey.key, args.index),
        &crate::id(),
        true,
        "ephemeral balance",
    )?;

    // Create the ephemeral balance PDA if it does not exist
    if ephemeral_balance_account.owner.eq(&system_program::id()) {
        create_pda(
            ephemeral_balance_account,
            &system_program::id(),
            8,
            ephemeral_balance_seeds_from_payer!(pubkey.key, args.index),
            bump_ephemeral_balance,
            system_program,
            payer,
        )?;
    }

    // Transfer lamports from payer to ephemeral PDA (with a system program call)
    if args.amount > 0 {
        let transfer_instruction = transfer(payer.key, ephemeral_balance_account.key, args.amount);
        invoke(
            &transfer_instruction,
            &[
                payer.clone(),
                ephemeral_balance_account.clone(),
                system_program.clone(),
            ],
        )?;
    }

    Ok(())
}
