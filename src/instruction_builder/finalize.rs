use solana_program::instruction::Instruction;
use solana_program::system_program;
use solana_program::{instruction::AccountMeta, pubkey::Pubkey};

use crate::discriminator::DlpDiscriminator;
use crate::pda::{
    commit_record_pda_from_pubkey, commit_state_pda_from_pubkey,
    delegation_metadata_pda_from_pubkey, delegation_record_pda_from_pubkey,
    validator_fees_vault_pda_from_pubkey,
};

/// Builds a finalize state instruction.
pub fn finalize(validator: Pubkey, delegated_account: Pubkey) -> Instruction {
    let delegation_record_pda = delegation_record_pda_from_pubkey(&delegated_account);
    let delegation_metadata_pda = delegation_metadata_pda_from_pubkey(&delegated_account);
    let commit_state_pda = commit_state_pda_from_pubkey(&delegated_account);
    let validator_fees_vault_pda = validator_fees_vault_pda_from_pubkey(&validator);
    let commit_record_pda = commit_record_pda_from_pubkey(&delegated_account);
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
        data: DlpDiscriminator::Finalize.to_vec(),
    }
}
