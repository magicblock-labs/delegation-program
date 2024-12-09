use crate::args::DelegateEphemeralBalanceArgs;
use crate::ephemeral_balance_seeds_from_payer;
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
    let ephemeral_balance_seeds = ephemeral_balance_seeds_from_payer!(pubkey.key, args.index);
    let (ephemeral_balance_address, ephemeral_balance_bump) =
        Pubkey::find_program_address(ephemeral_balance_seeds, &crate::id());
    if !ephemeral_balance_address.eq(delegate_account.key) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Set the delegation seeds
    args.delegate_args.seeds = ephemeral_balance_seeds.iter().map(|s| s.to_vec()).collect();

    // Generate the ephemeral balance PDA's signer seeds
    let ephemeral_balance_bump_slice = &[ephemeral_balance_bump];
    let ephemeral_balance_signer_seeds =
        [ephemeral_balance_seeds, &[ephemeral_balance_bump_slice]].concat();

    // Assign as owner the delegation program
    invoke_signed(
        &system_instruction::assign(delegate_account.key, &crate::id()),
        &[delegate_account.clone(), system_program.clone()],
        ephemeral_balance_signer_seeds,
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
        ephemeral_balance_signer_seeds,
    )?;

    Ok(())
}
