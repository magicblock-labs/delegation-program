use crate::args::CallHandlerArgs;
use crate::discriminator::DlpDiscriminator;
use crate::pda::{ephemeral_balance_pda_from_payer, validator_fees_vault_pda_from_validator};
use borsh::to_vec;
use solana_program::instruction::Instruction;
use solana_program::{instruction::AccountMeta, pubkey::Pubkey};

/// Builds a finalize state instruction.
/// See [crate::processor::call_handler] for docs.
pub fn call_handler(
    validator: Pubkey,
    handler_program: Pubkey,
    delegated_account: Pubkey, // TODO: rename
    other_accounts: Vec<AccountMeta>,
    args: CallHandlerArgs,
) -> Instruction {
    let validator_fees_vault_pda = validator_fees_vault_pda_from_validator(&validator);

    // handler accounts
    let escrow_account = ephemeral_balance_pda_from_payer(&delegated_account, args.escrow_index);
    let mut accounts = vec![
        AccountMeta::new(validator, true),
        AccountMeta::new(validator_fees_vault_pda, false),
        AccountMeta::new_readonly(handler_program, false),
        AccountMeta::new(delegated_account, false),
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
