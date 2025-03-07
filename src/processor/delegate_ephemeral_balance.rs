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

/// Delegates an account to transfer lamports which are used to fund it inside
/// the ephemeral.
///
/// Accounts:
///
/// 0. `[writable]` payer account
/// 1. `[signer]`   delegatee account from which the delegated account is derived
/// 2. `[writable]` ephemeral balance account
/// 3. `[writable]` delegate buffer PDA
/// 4. `[writable]` delegation record PDA
/// 5. `[writable]` delegation metadata PDA
/// 6. `[]`         system program
/// 7. `[]`         this program
///
/// Requirements:
///
/// - same as [crate::processor::delegate::process_delegate]
///
/// Steps:
///
/// 1. Delegates the ephemeral balance account to the delegation program so it can
///    act as an escrow
pub fn process_delegate_ephemeral_balance(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let mut args = DelegateEphemeralBalanceArgs::try_from_slice(data)?;
    let [payer, pubkey, ephemeral_balance_account, delegate_buffer, delegation_record, delegation_metadata, system_program, delegation_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    load_signer(payer, "payer")?;
    load_signer(pubkey, "delegatee")?;
    load_program(system_program, system_program::id(), "system program")?;
    load_program(delegation_program, crate::id(), "delegation program")?;

    // Check seeds and derive bump
    let ephemeral_balance_seeds: &[&[u8]] =
        ephemeral_balance_seeds_from_payer!(pubkey.key, args.index);
    let (ephemeral_balance_key, ephemeral_balance_bump) =
        Pubkey::find_program_address(ephemeral_balance_seeds, &crate::id());
    if !ephemeral_balance_key.eq(ephemeral_balance_account.key) {
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
        &system_instruction::assign(ephemeral_balance_account.key, &crate::id()),
        &[ephemeral_balance_account.clone(), system_program.clone()],
        &[&ephemeral_balance_signer_seeds],
    )?;

    // Create the delegation ix
    let ix = crate::instruction_builder::delegate(
        *payer.key,
        *ephemeral_balance_account.key,
        Some(crate::id()),
        args.delegate_args,
    );

    // Invoke signed delegation instruction
    invoke_signed(
        &ix,
        &[
            delegation_program.clone(),
            payer.clone(),
            ephemeral_balance_account.clone(),
            delegate_buffer.clone(),
            delegation_record.clone(),
            delegation_metadata.clone(),
            system_program.clone(),
        ],
        &[&ephemeral_balance_signer_seeds],
    )?;

    Ok(())
}
