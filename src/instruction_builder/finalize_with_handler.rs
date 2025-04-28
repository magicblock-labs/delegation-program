use borsh::to_vec;
use solana_program::instruction::Instruction;
use solana_program::system_program;
use solana_program::{instruction::AccountMeta, pubkey::Pubkey};
use crate::args::FinalizeWithDataArgs;
use crate::discriminator::DlpDiscriminator;
use crate::pda::{
    commit_record_pda_from_delegated_account, commit_state_pda_from_delegated_account,
    delegation_metadata_pda_from_delegated_account, delegation_record_pda_from_delegated_account,
    ephemeral_balance_pda_from_payer, validator_fees_vault_pda_from_validator,
};

/// Builds a finalize state instruction.
/// See [crate::processor::finalize_with_handler] for docs.
pub fn finalize_with_handler(
    validator: Pubkey,
    delegated_account: Pubkey,
    other_accounts: Vec<AccountMeta>,
    handler_program: Pubkey,
    args: FinalizeWithDataArgs,
) -> Instruction {
    // finalize accounts
    let commit_state_pda = commit_state_pda_from_delegated_account(&delegated_account);
    let commit_record_pda = commit_record_pda_from_delegated_account(&delegated_account);
    let delegation_record_pda = delegation_record_pda_from_delegated_account(&delegated_account);
    let delegation_metadata_pda =
        delegation_metadata_pda_from_delegated_account(&delegated_account);
    let validator_fees_vault_pda = validator_fees_vault_pda_from_validator(&validator);

    // handler accounts
    let escrow_account = ephemeral_balance_pda_from_payer(&delegated_account, args.escrow_index);
    let mut accounts = vec![
        // finalize accounts
        AccountMeta::new_readonly(validator, true),
        AccountMeta::new(delegated_account, false),
        AccountMeta::new(commit_state_pda, false),
        AccountMeta::new(commit_record_pda, false),
        AccountMeta::new(delegation_record_pda, false),
        AccountMeta::new(delegation_metadata_pda, false),
        AccountMeta::new(validator_fees_vault_pda, false),
        AccountMeta::new_readonly(system_program::id(), false),
        // handler accounts
        AccountMeta::new_readonly(handler_program, false),
        AccountMeta::new(escrow_account, false),
    ];
    // append other accounts at the end
    accounts.extend(other_accounts);

    Instruction {
        program_id: crate::id(),
        accounts,
        data: [
            DlpDiscriminator::FinalizeWithData.to_vec(),
            to_vec(&args).unwrap(),
        ]
        .concat(),
    }
}
