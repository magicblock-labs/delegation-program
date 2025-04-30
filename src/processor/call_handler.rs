use crate::args::{CallHandlerArgs};
use crate::consts::{
    EXTERNAL_CALL_HANDLER_DISCRIMINATOR,
};
use crate::ephemeral_balance_seeds_from_payer;
use crate::processor::utils::loaders::{
    load_initialized_validator_fees_vault, load_pda, load_signer,
};

use borsh::BorshDeserialize;
use solana_program::account_info::next_account_info;
use solana_program::account_info::AccountInfo;
use solana_program::entrypoint::ProgramResult;
use solana_program::instruction::{AccountMeta, Instruction};
use solana_program::msg;
use solana_program::program::invoke_signed;
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;

pub fn process_call_handler(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let args = CallHandlerArgs::try_from_slice(data)?;

    let accounts_iter = &mut accounts.iter();
    let validator = next_account_info(accounts_iter)?;
    let validator_fees_vault = next_account_info(accounts_iter)?;

    let handler_program = next_account_info(accounts_iter)?;
    // TODO: rename, actee or something like that
    let delegated_account = next_account_info(accounts_iter)?;
    let escrow_account = next_account_info(accounts_iter)?;

    // verify account is a signer
    load_signer(validator, "validator")?;
    // verify signer is a registered validator
    load_initialized_validator_fees_vault(validator, validator_fees_vault, true)?;

    // Check if destination prgram is executable
    if !handler_program.executable {
        msg!(
            "{} program is not executable: destination program",
            handler_program.key
        );
        return Err(ProgramError::InvalidAccountData);
    }
    // verify passed escrow_account derived from delegated_account
    let escrow_seeds: &[&[u8]] =
        ephemeral_balance_seeds_from_payer!(delegated_account.key, args.escrow_index);
    let escrow_bump = load_pda(
        escrow_account,
        escrow_seeds,
        &crate::id(),
        true,
        "ephemeral balance",
    )?;

    // deduce necessary accounts for CPI
    let (accounts_meta, handler_accounts): (Vec<AccountMeta>, Vec<AccountInfo>) =
        [delegated_account, escrow_account]
            .into_iter()
            .chain(accounts_iter)
            // TODO: check if we can keep it, but set is_signer false to prevent draining
            .filter(|account| account.key != validator.key)
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
    msg!(
        "Calling, accounts_meta.len: {}, handler_account.len: {}",
        accounts_meta.len(),
        handler_accounts.len()
    );

    let data = [EXTERNAL_CALL_HANDLER_DISCRIMINATOR.to_vec(), data.to_vec()].concat();
    let handler_instruction = Instruction {
        program_id: *handler_program.key,
        data,
        accounts: accounts_meta,
    };
    let bump_slice = &[escrow_bump];
    let escrow_signer_seeds = [escrow_seeds, &[bump_slice]].concat();

    invoke_signed(
        &handler_instruction,
        &handler_accounts,
        &[&escrow_signer_seeds],
    )
}
