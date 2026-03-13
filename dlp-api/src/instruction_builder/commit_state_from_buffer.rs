use borsh::to_vec;
use dlp::{
    args::CommitStateFromBufferArgs,
    discriminator::DlpDiscriminator,
    pda::{
        commit_record_pda_from_delegated_account,
        commit_state_pda_from_delegated_account,
        delegation_metadata_pda_from_delegated_account,
        delegation_record_pda_from_delegated_account,
        program_config_from_program_id,
        validator_fees_vault_pda_from_validator,
    },
    total_size_budget, AccountSizeClass, DLP_PROGRAM_DATA_SIZE_CLASS,
};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    system_program,
};

/// Builds a commit state from buffer instruction.
/// See [dlp::processor::process_commit_state_from_buffer] for docs.
pub fn commit_state_from_buffer(
    validator: Pubkey,
    delegated_account: Pubkey,
    delegated_account_owner: Pubkey,
    commit_state_buffer: Pubkey,
    commit_args: CommitStateFromBufferArgs,
) -> Instruction {
    let commit_args = to_vec(&commit_args).unwrap();
    let delegation_record_pda =
        delegation_record_pda_from_delegated_account(&delegated_account);
    let commit_state_pda =
        commit_state_pda_from_delegated_account(&delegated_account);
    let commit_record_pda =
        commit_record_pda_from_delegated_account(&delegated_account);
    let validator_fees_vault_pda =
        validator_fees_vault_pda_from_validator(&validator);
    let delegation_metadata_pda =
        delegation_metadata_pda_from_delegated_account(&delegated_account);
    let program_config_pda =
        program_config_from_program_id(&delegated_account_owner);
    Instruction {
        program_id: dlp::id(),
        accounts: vec![
            AccountMeta::new(validator, true),
            AccountMeta::new_readonly(delegated_account, false),
            AccountMeta::new(commit_state_pda, false),
            AccountMeta::new(commit_record_pda, false),
            AccountMeta::new_readonly(delegation_record_pda, false),
            AccountMeta::new(delegation_metadata_pda, false),
            AccountMeta::new_readonly(commit_state_buffer, false),
            AccountMeta::new_readonly(validator_fees_vault_pda, false),
            AccountMeta::new_readonly(program_config_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: [
            DlpDiscriminator::CommitStateFromBuffer.to_vec(),
            commit_args,
        ]
        .concat(),
    }
}

///
/// Returns accounts-data-size budget for commit_state_from_buffer instruction.
///
/// This value can be used with ComputeBudgetInstruction::SetLoadedAccountsDataSizeLimit
///
pub fn commit_state_from_buffer_size_budget(
    delegated_account: AccountSizeClass,
) -> u32 {
    total_size_budget(&[
        DLP_PROGRAM_DATA_SIZE_CLASS,
        AccountSizeClass::Tiny, // validator
        delegated_account,      // delegated_account
        delegated_account,      // commit_state_pda
        AccountSizeClass::Tiny, // commit_record_pda
        AccountSizeClass::Tiny, // delegation_record_pda
        AccountSizeClass::Tiny, // delegation_metadata_pda
        delegated_account,      // commit_state_buffer
        AccountSizeClass::Tiny, // validator_fees_vault_pda
        AccountSizeClass::Tiny, // program_config_pda
        AccountSizeClass::Tiny, // system_program
    ])
}
