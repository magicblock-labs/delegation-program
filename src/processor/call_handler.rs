use crate::args::CallHandlerArgs;
use crate::consts::EXTERNAL_CALL_HANDLER_DISCRIMINATOR;
use crate::ephemeral_balance_seeds_from_payer;
use crate::processor::utils::loaders::{
    load_initialized_validator_fees_vault, load_owned_pda, load_pda, load_signer,
};

use borsh::BorshDeserialize;
use solana_program::account_info::AccountInfo;
use solana_program::entrypoint::ProgramResult;
use solana_program::instruction::{AccountMeta, Instruction};
use solana_program::program::invoke_signed;
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;
use solana_program::{msg, system_program};

pub const INVALID_ESCROW_PDA: &str = "invalid escrow pda in CallHandler";
pub const INVALID_ESCROW_OWNER: &str = "escrow can not be delegated in CallHandler";

/// Calls a handler on user specified program
///
/// Accounts:
/// 0: `[signer]`   validator
/// 1: `[]`         validator fee vault to verify its registration
/// 2: `[]`         destination program of an action
/// 3: `[]`         escrow authority account which created escrow account
/// 4: `[writable]` non delegated escrow pda created from 3
/// 5: `[readonly/writable]` other accounts needed for action
/// 6: `[readonly/writable]` other accounts needed for action
/// 7: ...
///
/// Requirements:
///
/// - escrow account initialized
/// - escrow account not delegated
/// - validator as a caller
///
/// Steps:
/// 1. Verify that signer is a valid registered validator
/// 2. Verify escrow pda exists and not delegated
/// 3. Invoke signed on behalf of escrow pda user specified action
///
/// Usage:
///
/// This instruction is meant to be called via CPI with the owning program signing for the
/// delegated account.
pub fn process_call_handler(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    const OTHER_ACCOUNTS_OFFSET: usize = 5;

    if accounts.len() < OTHER_ACCOUNTS_OFFSET {
        return Err(ProgramError::NotEnoughAccountKeys);
    }

    let (
        [validator, validator_fees_vault, destination_program, escrow_authority_account, escrow_account],
        other_accounts,
    ) = accounts.split_at(OTHER_ACCOUNTS_OFFSET)
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    let args = CallHandlerArgs::try_from_slice(data)?;

    // verify account is a signer
    load_signer(validator, "validator")?;
    // verify signer is a registered validator
    load_initialized_validator_fees_vault(validator, validator_fees_vault, true)?;
    // Check if destination program is executable
    if !destination_program.executable {
        msg!(
            "{} program is not executable: destination program",
            destination_program.key
        );
        return Err(ProgramError::InvalidAccountData);
    }

    // verify passed escrow_account derived from escrow authority
    let escrow_seeds: &[&[u8]] =
        ephemeral_balance_seeds_from_payer!(escrow_authority_account.key, args.escrow_index);
    let escrow_bump = load_pda(
        escrow_account,
        escrow_seeds,
        &crate::id(),
        true,
        INVALID_ESCROW_PDA,
    )?;
    load_owned_pda(escrow_account, &system_program::id(), INVALID_ESCROW_OWNER)?;

    // deduce necessary accounts for CPI
    let (accounts_meta, handler_accounts): (Vec<AccountMeta>, Vec<AccountInfo>) =
        [escrow_authority_account, escrow_account]
            .into_iter()
            .chain(other_accounts)
            .filter(|account| account.key != validator.key)
            .map(|account| {
                (
                    // We enable only escrow to be a signer
                    AccountMeta {
                        pubkey: *account.key,
                        is_writable: account.is_writable,
                        is_signer: account.key == escrow_account.key,
                    },
                    account.clone(),
                )
            })
            .collect();

    let data = [EXTERNAL_CALL_HANDLER_DISCRIMINATOR.to_vec(), data.to_vec()].concat();
    let handler_instruction = Instruction {
        program_id: *destination_program.key,
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
