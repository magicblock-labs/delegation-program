use crate::args::TopUpEphemeralBalanceArgs;
use crate::consts::EPHEMERAL_BALANCE;
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
    let [payer, ephemeral_balance_pda, system_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    load_signer(payer)?;
    load_program(system_program, system_program::id())?;

    let seeds_ephemeral_balance_pda = [EPHEMERAL_BALANCE, &payer.key.to_bytes(), &[args.index]];
    let bump_ephemeral_balance = load_pda(
        ephemeral_balance_pda,
        &seeds_ephemeral_balance_pda,
        &crate::id(),
        true,
    )?;

    // Create the ephemeral balance PDA if it does not exist
    if ephemeral_balance_pda.owner.eq(&system_program::id()) {
        create_pda(
            ephemeral_balance_pda,
            &system_program::id(),
            8,
            &[
                EPHEMERAL_BALANCE,
                &payer.key.to_bytes(),
                &[args.index],
                &[bump_ephemeral_balance],
            ],
            system_program,
            payer,
        )?;
    }

    // Transfer lamports from payer to ephemeral PDA (with a system program call)
    if args.amount > 0 {
        let transfer_instruction = transfer(payer.key, ephemeral_balance_pda.key, args.amount);
        invoke(
            &transfer_instruction,
            &[
                payer.clone(),
                ephemeral_balance_pda.clone(),
                system_program.clone(),
            ],
        )?;
    }

    Ok(())
}
