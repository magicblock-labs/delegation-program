use dlp::{
    discriminator::DlpDiscriminator,
    pda::{
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
use solana_sdk_ids::system_program;

use crate::compat::{Compatize, Modernize};

/// Builds a request undelegation instruction.
/// See [dlp::processor::process_request_undelegation] for docs.
pub fn request_undelegation(
    payer: Pubkey,
    delegated_account: Pubkey,
    owner_program: Pubkey,
) -> Instruction {
    let delegated_account_compat = delegated_account.compatize();
    let undelegation_request_pda =
        undelegation_request_pda_from_delegated_account(
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
    Instruction {
        program_id: dlp::id().modernize(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(delegated_account, true),
            AccountMeta::new_readonly(owner_program, false),
            AccountMeta::new(undelegation_request_pda, false),
            AccountMeta::new_readonly(delegation_record_pda, false),
            AccountMeta::new_readonly(delegation_metadata_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: DlpDiscriminator::RequestUndelegation.to_vec(),
    }
}

///
/// Returns accounts-data-size budget for request undelegation instruction.
///
/// This value can be used with ComputeBudgetInstruction::SetLoadedAccountsDataSizeLimit
///
pub fn request_undelegation_size_budget(
    delegated_account: AccountSizeClass,
) -> u32 {
    total_size_budget(&[
        DLP_PROGRAM_DATA_SIZE_CLASS,
        AccountSizeClass::Tiny, // payer
        delegated_account,      // delegated_account
        AccountSizeClass::Tiny, // owner_program
        AccountSizeClass::Tiny, // undelegation_request_pda
        AccountSizeClass::Tiny, // delegation_record_pda
        AccountSizeClass::Tiny, // delegation_metadata_pda
        AccountSizeClass::Tiny, // system_program
    ])
}
