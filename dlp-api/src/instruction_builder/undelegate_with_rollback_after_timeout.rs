use dlp::{
    discriminator::DlpDiscriminator,
    pda::{
        commit_record_pda_from_delegated_account,
        commit_state_pda_from_delegated_account,
        delegation_metadata_pda_from_delegated_account,
        delegation_record_pda_from_delegated_account,
        undelegation_request_pda_from_delegated_account,
    },
    total_size_budget, AccountSizeClass, DLP_PROGRAM_DATA_SIZE_CLASS,
};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

use crate::compat::{Compatize, Modernize};

/// Builds an owner-program-authorized timeout rollback instruction for a
/// requested undelegation.
/// See [dlp::processor::process_undelegate_with_rollback_after_timeout] for docs.
pub fn undelegate_with_rollback_after_timeout(
    delegated_account: Pubkey,
    owner_program: Pubkey,
    delegation_rent_payer: Pubkey,
    commit_reimbursement: Pubkey,
) -> Instruction {
    let delegated_account_compat = delegated_account.compatize();
    let request_pda = undelegation_request_pda_from_delegated_account(
        &delegated_account_compat,
    )
    .modernize();
    let delegation_record_pda =
        delegation_record_pda_from_delegated_account(&delegated_account_compat)
            .modernize();
    let delegation_metadata_pda =
        delegation_metadata_pda_from_delegated_account(
            &delegated_account_compat,
        )
        .modernize();
    let commit_state_pda =
        commit_state_pda_from_delegated_account(&delegated_account_compat)
            .modernize();
    let commit_record_pda =
        commit_record_pda_from_delegated_account(&delegated_account_compat)
            .modernize();

    Instruction {
        program_id: dlp::id().modernize(),
        accounts: vec![
            AccountMeta::new(delegated_account, true),
            AccountMeta::new_readonly(owner_program, false),
            AccountMeta::new(request_pda, false),
            AccountMeta::new(delegation_record_pda, false),
            AccountMeta::new(delegation_metadata_pda, false),
            AccountMeta::new(delegation_rent_payer, false),
            AccountMeta::new(commit_state_pda, false),
            AccountMeta::new(commit_record_pda, false),
            AccountMeta::new(commit_reimbursement, false),
        ],
        data: DlpDiscriminator::UndelegateWithRollbackAfterTimeout.to_vec(),
    }
}

///
/// Returns accounts-data-size budget for undelegate-with-rollback-after-timeout.
///
/// This value can be used with ComputeBudgetInstruction::SetLoadedAccountsDataSizeLimit
///
pub fn undelegate_with_rollback_after_timeout_size_budget(
    delegated_account: AccountSizeClass,
) -> u32 {
    total_size_budget(&[
        DLP_PROGRAM_DATA_SIZE_CLASS,
        delegated_account,      // delegated_account
        AccountSizeClass::Tiny, // owner_program
        AccountSizeClass::Tiny, // undelegation_request_pda
        AccountSizeClass::Tiny, // delegation_record_pda
        AccountSizeClass::Tiny, // delegation_metadata_pda
        AccountSizeClass::Tiny, // delegation_rent_payer
        delegated_account,      // commit_state_pda
        AccountSizeClass::Tiny, // commit_record_pda
        AccountSizeClass::Tiny, // commit_reimbursement
    ])
}
