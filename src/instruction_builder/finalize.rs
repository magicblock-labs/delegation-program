use solana_program::instruction::Instruction;
use solana_program::system_program;
use solana_program::{instruction::AccountMeta, pubkey::Pubkey};

use crate::discriminant::DlpDiscriminant;
use crate::pda::{
    commit_record_pda_from_delegated_account, commit_state_pda_from_delegated_account,
    delegation_metadata_pda_from_delegated_account, delegation_record_pda_from_delegated_account,
    validator_fees_vault_pda_from_validator,
};

/// Builds a finalize state instruction.
pub fn finalize(validator: Pubkey, delegated_account: Pubkey) -> Instruction {
    let commit_state_pda = commit_state_pda_from_delegated_account(&delegated_account);
    let commit_record_pda = commit_record_pda_from_delegated_account(&delegated_account);
    let delegation_record_pda = delegation_record_pda_from_delegated_account(&delegated_account);
    let delegation_metadata_pda =
        delegation_metadata_pda_from_delegated_account(&delegated_account);
    let validator_fees_vault_pda = validator_fees_vault_pda_from_validator(&validator);
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new_readonly(validator, true),
            AccountMeta::new(delegated_account, false),
            AccountMeta::new(commit_state_pda, false),
            AccountMeta::new(commit_record_pda, false),
            AccountMeta::new(delegation_record_pda, false),
            AccountMeta::new(delegation_metadata_pda, false),
            AccountMeta::new(validator_fees_vault_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: DlpDiscriminant::Finalize.to_vec(),
    }
}
