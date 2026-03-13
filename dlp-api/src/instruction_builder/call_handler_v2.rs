use borsh::to_vec;
use dlp::{
    args::CallHandlerArgs,
    discriminator::DlpDiscriminator,
    pda::{
        ephemeral_balance_pda_from_payer,
        validator_fees_vault_pda_from_validator,
    },
    total_size_budget, AccountSizeClass, DLP_PROGRAM_DATA_SIZE_CLASS,
};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

/// Builds a call handler v2 instruction.
/// See [dlp::processor::call_handler_v2] for docs.
pub fn call_handler_v2(
    validator: Pubkey,
    destination_program: Pubkey,
    source_program: Pubkey,
    escrow_authority: Pubkey,
    other_accounts: Vec<AccountMeta>,
    args: CallHandlerArgs,
) -> Instruction {
    let validator_fees_vault_pda =
        validator_fees_vault_pda_from_validator(&validator);

    // handler accounts
    let escrow_account =
        ephemeral_balance_pda_from_payer(&escrow_authority, args.escrow_index);
    let mut accounts = vec![
        AccountMeta::new(validator, true),
        AccountMeta::new(validator_fees_vault_pda, false),
        AccountMeta::new_readonly(destination_program, false),
        AccountMeta::new_readonly(source_program, false),
        AccountMeta::new(escrow_authority, false),
        AccountMeta::new(escrow_account, false),
    ];
    // append other accounts at the end
    accounts.extend(other_accounts);

    Instruction {
        program_id: dlp::id(),
        accounts,
        data: [
            DlpDiscriminator::CallHandlerV2.to_vec(),
            to_vec(&args).unwrap(),
        ]
        .concat(),
    }
}

///
/// Returns accounts-data-size budget for call_handler_v2 instruction.
///
/// This value can be used with ComputeBudgetInstruction::SetLoadedAccountsDataSizeLimit
///
pub fn call_handler_v2_size_budget(
    destination_program: AccountSizeClass,
    source_program: AccountSizeClass,
    other_accounts: u32,
) -> u32 {
    total_size_budget(&[
        DLP_PROGRAM_DATA_SIZE_CLASS,
        AccountSizeClass::Tiny, // validator
        AccountSizeClass::Tiny, // validator_fees_vault_pda
        destination_program,
        source_program,
        AccountSizeClass::Tiny, // escrow_authority
        AccountSizeClass::Tiny, // escrow_account
    ]) + other_accounts
}
