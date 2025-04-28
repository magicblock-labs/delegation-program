use crate::args::FinalizeWithDataArgs;
use crate::ephemeral_balance_seeds_from_payer;
use crate::instruction_builder::delegate;
use crate::processor::process_finalize;
use crate::processor::utils::loaders::load_pda;
use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::account_info::next_account_info;
use solana_program::account_info::AccountInfo;
use solana_program::entrypoint::ProgramResult;
use solana_program::instruction::{AccountMeta, Instruction};
use solana_program::msg;
use solana_program::program::invoke_signed;
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;

// TODO: this actually could be encoded as Instruction
// Client creates Instruction where specifies destination_program
// encodes how to call it and adds necessary data
#[derive(BorshSerialize, BorshDeserialize)]
pub enum HandleIx {
    FinalizeWithDataHandler(Vec<u8>),
    UndelegateWithDataHandler(Vec<u8>),
}

pub fn process_finalize_with_data(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    const FINALIZE_ACCOUNTS_SIZE: usize = 8;
    const HANDLER_ACCOUNTS_SIZE: usize = 2;

    let args = FinalizeWithDataArgs::try_from_slice(data)?;
    let (finalize_accounts, remaining_accounts) = if accounts.len() >= FINALIZE_ACCOUNTS_SIZE {
        accounts.split_at(FINALIZE_ACCOUNTS_SIZE)
    } else {
        return Err(ProgramError::NotEnoughAccountKeys.into());
    };
    let (handler_accounts, remaining_accounts) =
        if remaining_accounts.len() >= HANDLER_ACCOUNTS_SIZE {
            remaining_accounts.split_at(HANDLER_ACCOUNTS_SIZE)
        } else {
            return Err(ProgramError::NotEnoughAccountKeys.into());
        };

    let accounts_iter = &mut handler_accounts.iter();
    let destination_program = next_account_info(accounts_iter)?;
    let escrow_account = next_account_info(accounts_iter)?;

    // Check if destination prgram is executable
    if !destination_program.executable {
        msg!(
            "{} program is not executable: destination program",
            destination_program.key
        );
        return Err(ProgramError::InvalidAccountData);
    }

    // verify passed escrow_account derived from delegated_account
    let delegated_account = finalize_accounts[1].clone();
    let escrow_seeds: &[&[u8]] =
        ephemeral_balance_seeds_from_payer!(delegated_account.key, args.escrow_index);
    let escrow_bump = load_pda(
        escrow_account,
        escrow_seeds,
        &crate::id(),
        true,
        "ephemeral balance",
    )?;

    // Finalize first
    process_finalize(program_id, finalize_accounts, data)?;

    // deduce necessary accounts for CPI
    let validator_account = finalize_accounts[0].clone();
    let (accounts_meta, handler_accounts): (Vec<AccountMeta>, Vec<AccountInfo>) =
        [delegated_account, escrow_account.clone()]
            .iter()
            .chain(remaining_accounts)
            .filter(|account| account.key != validator_account.key)
            .map(|account| {
                (
                    AccountMeta {
                        pubkey: *account.key,
                        is_writable: account.is_writable,
                        is_signer: account.key == escrow_account.key,
                    },
                    account.clone(),
                )
            })
            .collect();
    msg!("Calling, accounts_meta.len: {}, handler_account.len: {}", accounts_meta.len(), handler_accounts.len());

    let handler_instruction = Instruction::new_with_borsh(
        *destination_program.key,
        &HandleIx::FinalizeWithDataHandler(args.data),
        accounts_meta,
    );
    let bump_slice = &[escrow_bump];
    let escrow_signer_seeds = [escrow_seeds, &[bump_slice]].concat();
    invoke_signed(
        &handler_instruction,
        &handler_accounts,
        &[&escrow_signer_seeds],
    )?;

    Ok(())
}
