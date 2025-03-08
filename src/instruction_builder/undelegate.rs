use solana_program::instruction::Instruction;
use solana_program::system_program;
use solana_program::{instruction::AccountMeta, pubkey::Pubkey};

use crate::discriminator::DlpDiscriminator;
use crate::pda::{
    commit_record_pda_from_delegated_account, commit_state_pda_from_delegated_account,
    delegation_metadata_pda_from_delegated_account, delegation_record_pda_from_delegated_account,
    fees_vault_pda, undelegate_buffer_pda_from_delegated_account,
    validator_fees_vault_pda_from_validator,
};

/// Builds an undelegate instruction.
/// See [crate::processor::process_undelegate] for docs.
#[allow(clippy::too_many_arguments)]
pub fn undelegate(
    validator: Pubkey,
    delegated_account: Pubkey,
    owner_program: Pubkey,
    rent_reimbursement: Pubkey,
) -> Instruction {
    let undelegate_buffer_pda = undelegate_buffer_pda_from_delegated_account(&delegated_account);
    let commit_state_pda = commit_state_pda_from_delegated_account(&delegated_account);
    let commit_record_pda = commit_record_pda_from_delegated_account(&delegated_account);
    let delegation_record_pda = delegation_record_pda_from_delegated_account(&delegated_account);
    let delegation_metadata_pda =
        delegation_metadata_pda_from_delegated_account(&delegated_account);
    let fees_vault_pda = fees_vault_pda();
    let validator_fees_vault_pda = validator_fees_vault_pda_from_validator(&validator);
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(validator, true),
            AccountMeta::new(delegated_account, false),
            AccountMeta::new_readonly(owner_program, false),
            AccountMeta::new(undelegate_buffer_pda, false),
            AccountMeta::new_readonly(commit_state_pda, false),
            AccountMeta::new_readonly(commit_record_pda, false),
            AccountMeta::new(delegation_record_pda, false),
            AccountMeta::new(delegation_metadata_pda, false),
            AccountMeta::new(rent_reimbursement, false),
            AccountMeta::new(fees_vault_pda, false),
            AccountMeta::new(validator_fees_vault_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: DlpDiscriminator::Undelegate.to_vec(),
    }
}
