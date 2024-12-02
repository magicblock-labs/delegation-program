use crate::consts::EPHEMERAL_BALANCE;
use crate::instruction::{DelegateTopUpAccountArgs, TopUpEphemeralArgs};
use crate::utils::loaders::{load_initialized_pda, load_pda, load_program, load_signer};
use crate::utils::utils_pda::{close_pda, create_pda};
use borsh::BorshDeserialize;
use solana_program::program::{invoke, invoke_signed};
use solana_program::program_error::ProgramError;
use solana_program::system_instruction::transfer;
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey, system_instruction,
    system_program,
};

pub fn process_top_up(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    // Parse args.
    let args = TopUpEphemeralArgs::try_from_slice(data)?;

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

pub fn process_delegate_ephemeral_balance(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let mut args = DelegateTopUpAccountArgs::try_from_slice(data)?;
    let [payer, delegate_account, buffer, delegation_record, delegation_metadata, system_program, delegation_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    load_signer(payer)?;
    load_program(system_program, system_program::id())?;

    // Check seeds and derive bump
    let seeds = &[EPHEMERAL_BALANCE, &payer.key.to_bytes(), &[args.index]];
    let (address, bump) = Pubkey::find_program_address(seeds, &crate::id());
    if !address.eq(delegate_account.key) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Set the delegation seeds
    args.delegate_args.seeds = seeds.iter().map(|s| s.to_vec()).collect();

    // Assign as owner the delegation program
    invoke_signed(
        &system_instruction::assign(delegate_account.key, &crate::id()),
        &[delegate_account.clone(), system_program.clone()],
        &[&[
            EPHEMERAL_BALANCE,
            &payer.key.to_bytes(),
            &[args.index],
            &[bump],
        ]],
    )?;

    // Create the delegation ix
    let ix = crate::instruction::delegate(
        *payer.key,
        *delegate_account.key,
        Some(crate::id()),
        args.delegate_args,
    );

    // Invoke signed delegation instruction
    invoke_signed(
        &ix,
        &[
            delegation_program.clone(),
            payer.clone(),
            delegate_account.clone(),
            buffer.clone(),
            delegation_record.clone(),
            delegation_metadata.clone(),
            system_program.clone(),
        ],
        &[&[
            EPHEMERAL_BALANCE,
            &payer.key.to_bytes(),
            &[args.index],
            &[bump],
        ]],
    )?;

    Ok(())
}

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
        &[EPHEMERAL_BALANCE, &payer.key.to_bytes(), &[index]],
        &crate::id(),
        true,
    )?;

    close_pda(ephemeral_balance_pda, payer)?;

    Ok(())
}
