use crate::args::CallHandlerArgs;
use crate::discriminator::DlpDiscriminator;
use crate::pda::{ephemeral_balance_pda_from_payer, validator_fees_vault_pda_from_validator};
use crate::{total_size_budget, AccountSizeClass, DLP_PROGRAM_DATA_SIZE_CLASS};
use borsh::to_vec;
use solana_program::instruction::Instruction;
use solana_program::{instruction::AccountMeta, pubkey::Pubkey};

/// Builds a call handler instruction.
/// See [crate::processor::call_handler] for docs.
pub fn call_handler(
    validator: Pubkey,
    destination_program: Pubkey,
    escrow_authority: Pubkey,
    other_accounts: Vec<AccountMeta>,
    args: CallHandlerArgs,
) -> Instruction {
    let validator_fees_vault_pda = validator_fees_vault_pda_from_validator(&validator);

    // handler accounts
    let escrow_account = ephemeral_balance_pda_from_payer(&escrow_authority, args.escrow_index);
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
        program_id: crate::id(),
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
pub fn call_handler_size_budget(destination_program: AccountSizeClass, other_accounts: u32) -> u32 {
    total_size_budget(&[
        DLP_PROGRAM_DATA_SIZE_CLASS,
        AccountSizeClass::Tiny, // validator
        AccountSizeClass::Tiny, // validator_fees_vault_pda
        destination_program,
        AccountSizeClass::Tiny, // escrow_authority
        AccountSizeClass::Tiny, // escrow_account
    ]) + other_accounts
}
