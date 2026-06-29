use dlp::{
    discriminator::DlpDiscriminator,
    pda::{
        commit_record_pda_from_delegated_account,
        commit_state_pda_from_delegated_account,
        delegation_metadata_pda_from_delegated_account,
        delegation_record_pda_from_delegated_account, fees_vault_pda,
        undelegate_buffer_pda_from_delegated_account,
        undelegation_request_pda_from_delegated_account,
        validator_fees_vault_pda_from_validator,
    },
    total_size_budget, AccountSizeClass, DLP_PROGRAM_DATA_SIZE_CLASS,
};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;

use crate::compat::{Compatize, Modernize};

/// Builds a finalize state instruction.
/// See [dlp::processor::process_finalize] for docs.
pub fn finalize(
    validator: Pubkey,
    delegated_account: Pubkey,
    owner_program: Pubkey,
    rent_reimbursement: Pubkey,
) -> Instruction {
    let validator_compat = validator.compatize();
    let delegated_account_compat = delegated_account.compatize();
    let commit_state_pda =
        commit_state_pda_from_delegated_account(&delegated_account_compat)
            .modernize();
    let commit_record_pda =
        commit_record_pda_from_delegated_account(&delegated_account_compat)
            .modernize();
    let delegation_record_pda =
        delegation_record_pda_from_delegated_account(&delegated_account_compat)
            .modernize();
    let delegation_metadata_pda =
        delegation_metadata_pda_from_delegated_account(
            &delegated_account_compat,
        )
        .modernize();
    let fees_vault_pda = fees_vault_pda().modernize();
    let undelegate_buffer_pda =
        undelegate_buffer_pda_from_delegated_account(&delegated_account_compat)
            .modernize();
    let undelegation_request_pda =
        undelegation_request_pda_from_delegated_account(
            &delegated_account_compat,
        )
        .modernize();
    let validator_fees_vault_pda =
        validator_fees_vault_pda_from_validator(&validator_compat).modernize();
    Instruction {
        program_id: dlp::id().modernize(),
        accounts: vec![
            AccountMeta::new(validator, true),
            AccountMeta::new(delegated_account, false),
            AccountMeta::new(commit_state_pda, false),
            AccountMeta::new(commit_record_pda, false),
            AccountMeta::new(delegation_record_pda, false),
            AccountMeta::new(delegation_metadata_pda, false),
            AccountMeta::new(validator_fees_vault_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(owner_program, false),
            AccountMeta::new(undelegate_buffer_pda, false),
            AccountMeta::new(rent_reimbursement, false),
            AccountMeta::new(fees_vault_pda, false),
            AccountMeta::new(undelegation_request_pda, false),
        ],
        data: DlpDiscriminator::Finalize.to_vec(),
    }
}

///
/// Returns accounts-data-size budget for finalize instruction.
///
/// This value can be used with ComputeBudgetInstruction::SetLoadedAccountsDataSizeLimit
///
pub fn finalize_size_budget(delegated_account: AccountSizeClass) -> u32 {
    total_size_budget(&[
        DLP_PROGRAM_DATA_SIZE_CLASS,
        AccountSizeClass::Tiny, // validator
        delegated_account,      // delegated_account
        delegated_account,      // commit_state_pda
        AccountSizeClass::Tiny, // commit_record_pda
        AccountSizeClass::Tiny, // delegation_record_pda
        AccountSizeClass::Tiny, // delegation_metadata_pda
        AccountSizeClass::Tiny, // validator_fees_vault_pda
        AccountSizeClass::Tiny, // system_program
        AccountSizeClass::Tiny, // owner_program
        delegated_account,      // undelegate_buffer_pda
        AccountSizeClass::Tiny, // rent_reimbursement
        AccountSizeClass::Tiny, // fees_vault_pda
        AccountSizeClass::Tiny, // undelegation_request_pda
    ])
}
