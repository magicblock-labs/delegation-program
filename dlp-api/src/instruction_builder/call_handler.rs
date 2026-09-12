use dlp::{
    args::CallHandlerArgs,
    consts::ACTION_EXECUTOR_PROGRAM_ID,
    discriminator::DlpDiscriminator,
    pda::{
        action_executor_escrow_pda_from_authority,
        ephemeral_balance_pda_from_payer,
        validator_fees_vault_pda_from_validator,
    },
    total_size_budget, AccountSizeClass, DLP_PROGRAM_DATA_SIZE_CLASS,
};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

use crate::compat::{borsh::to_vec, Compatize, Modernize};

/// Builds a call handler instruction.
/// See [dlp::processor::call_handler] for docs.
#[deprecated(since = "1.1.4", note = "Use `call_handler_v2` instead")]
pub fn call_handler(
    validator: Pubkey,
    destination_program: Pubkey,
    escrow_authority: Pubkey,
    other_accounts: Vec<AccountMeta>,
    args: CallHandlerArgs,
) -> Instruction {
    let validator_compat = validator.compatize();
    let validator_fees_vault_pda =
        validator_fees_vault_pda_from_validator(&validator_compat).modernize();

    // handler accounts
    let escrow_authority_compat = escrow_authority.compatize();
    let escrow_account = ephemeral_balance_pda_from_payer(
        &escrow_authority_compat,
        args.escrow_index,
    )
    .modernize();
    let mut accounts = vec![
        AccountMeta::new(validator, true),
        AccountMeta::new(validator_fees_vault_pda, false),
        AccountMeta::new_readonly(destination_program, false),
        AccountMeta::new(escrow_authority, false),
        AccountMeta::new(escrow_account, false),
    ];
    // append other accounts at the end
    accounts.extend(other_accounts);

    Instruction {
        program_id: dlp::id().modernize(),
        accounts,
        data: [
            DlpDiscriminator::CallHandler.to_vec(),
            to_vec(&args).unwrap(),
        ]
        .concat(),
    }
}

///
/// Returns accounts-data-size budget for call_handler instruction.
///
/// This value can be used with ComputeBudgetInstruction::SetLoadedAccountsDataSizeLimit
///
pub fn call_handler_size_budget(
    destination_program: AccountSizeClass,
    other_accounts: u32,
) -> u32 {
    total_size_budget(&[
        DLP_PROGRAM_DATA_SIZE_CLASS,
        AccountSizeClass::Tiny, // validator
        AccountSizeClass::Tiny, // validator_fees_vault_pda
        destination_program,
        AccountSizeClass::Tiny, // escrow_authority
        AccountSizeClass::Tiny, // escrow_account
    ]) + other_accounts
}

/// Builds an action-executor instruction.
///
/// Account layout matches [`call_handler`], except the escrow is the
/// action-executor PDA (not DLP's ephemeral balance). The executor signs that
/// escrow for the destination CPI; the validator is never a destination
/// signer.
pub fn execute_action(
    validator: Pubkey,
    destination_program: Pubkey,
    escrow_authority: Pubkey,
    other_accounts: Vec<AccountMeta>,
    args: CallHandlerArgs,
) -> Instruction {
    let validator_compat = validator.compatize();
    let validator_fees_vault_pda =
        validator_fees_vault_pda_from_validator(&validator_compat).modernize();
    let escrow_authority_compat = escrow_authority.compatize();
    let escrow_account = action_executor_escrow_pda_from_authority(
        &escrow_authority_compat,
        args.escrow_index,
    )
    .modernize();
    let mut accounts = vec![
        AccountMeta::new(validator, true),
        AccountMeta::new(validator_fees_vault_pda, false),
        AccountMeta::new_readonly(destination_program, false),
        AccountMeta::new(escrow_authority, false),
        AccountMeta::new(escrow_account, true),
    ];
    accounts.extend(other_accounts);

    Instruction {
        program_id: ACTION_EXECUTOR_PROGRAM_ID.modernize(),
        accounts,
        data: [
            DlpDiscriminator::CallHandler.to_vec(),
            to_vec(&args).unwrap(),
        ]
        .concat(),
    }
}

/// Accounts-data-size budget for [`execute_action`].
pub fn execute_action_size_budget(
    destination_program: AccountSizeClass,
    other_accounts: u32,
) -> u32 {
    call_handler_size_budget(destination_program, other_accounts)
}
