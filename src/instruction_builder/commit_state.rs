use borsh::BorshSerialize;
use solana_program::instruction::Instruction;
use solana_program::system_program;
use solana_program::{instruction::AccountMeta, pubkey::Pubkey};

use crate::args::CommitStateArgs;
use crate::discriminant::DlpDiscriminant;
use crate::pda::{
    committed_state_pda_from_pubkey, committed_state_record_pda_from_pubkey,
    delegation_metadata_pda_from_pubkey, delegation_record_pda_from_pubkey,
    program_config_pda_from_pubkey, validator_fees_vault_pda_from_pubkey,
};

/// Builds a commit state instruction.
pub fn commit_state(
    validator: Pubkey,
    delegated_account: Pubkey,
    delegated_account_owner: Pubkey,
    commit_args: CommitStateArgs,
) -> Instruction {
    let commit_args = commit_args.try_to_vec().unwrap();
    let delegation_record_pda = delegation_record_pda_from_pubkey(&delegated_account);
    let commit_state_pda = committed_state_pda_from_pubkey(&delegated_account);
    let commit_state_record_pda = committed_state_record_pda_from_pubkey(&delegated_account);
    let validator_fees_vault_pda = validator_fees_vault_pda_from_pubkey(&validator);
    let delegation_metadata_pda = delegation_metadata_pda_from_pubkey(&delegated_account);
    let whitelist_program_config = program_config_pda_from_pubkey(&delegated_account_owner);
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new_readonly(validator, true),
            AccountMeta::new_readonly(delegated_account, false),
            AccountMeta::new(commit_state_pda, false),
            AccountMeta::new(commit_state_record_pda, false),
            AccountMeta::new(delegation_record_pda, false),
            AccountMeta::new(delegation_metadata_pda, false),
            AccountMeta::new(validator_fees_vault_pda, false),
            AccountMeta::new_readonly(whitelist_program_config, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: [DlpDiscriminant::CommitState.to_vec(), commit_args].concat(),
    }
}
