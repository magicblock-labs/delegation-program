use crate::args::DelegateEphemeralBalanceArgs;
use crate::processor::utils::loaders::{load_program, load_signer};
use borsh::BorshDeserialize;
use solana_program::program::invoke_signed;
use solana_program::program_error::ProgramError;
use solana_program::system_program;
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey, system_instruction,
};

pub fn process_delegate_ephemeral_balance(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let mut args = DelegateEphemeralBalanceArgs::try_from_slice(data)?;
    let [payer, pubkey, delegate_account, buffer, delegation_record, delegation_metadata, system_program, delegation_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    load_signer(payer)?;
    load_signer(pubkey)?;
    load_program(system_program, system_program::id())?;
    load_program(delegation_program, crate::id())?;

    // Check seeds and derive bump
    let seeds = &[EPHEMERAL_BALANCE, &pubkey.key.to_bytes(), &[args.index]];
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
            &pubkey.key.to_bytes(),
            &[args.index],
            &[bump],
        ]],
    )?;

    // Create the delegation ix
    let ix = crate::instruction_builder::delegate(
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
            &pubkey.key.to_bytes(),
            &[args.index],
            &[bump],
        ]],
    )?;

    Ok(())
}
