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

/// Builds an undelegate instruction.
/// See [dlp::processor::process_undelegate] for docs.
#[allow(clippy::too_many_arguments)]
pub fn undelegate(
    validator: Pubkey,
    delegated_account: Pubkey,
    owner_program: Pubkey,
    rent_reimbursement: Pubkey,
) -> Instruction {
    build_undelegate(
        validator,
        delegated_account,
        owner_program,
        rent_reimbursement,
        None,
    )
}

/// Builds an undelegate instruction that also closes a matching request.
/// See [dlp::processor::process_undelegate] for docs.
#[allow(clippy::too_many_arguments)]
pub fn undelegate_with_request(
    validator: Pubkey,
    delegated_account: Pubkey,
    owner_program: Pubkey,
    rent_reimbursement: Pubkey,
    request_rent_payer: Pubkey,
) -> Instruction {
    build_undelegate(
        validator,
        delegated_account,
        owner_program,
        rent_reimbursement,
        Some(request_rent_payer),
    )
}

fn build_undelegate(
    validator: Pubkey,
    delegated_account: Pubkey,
    owner_program: Pubkey,
    rent_reimbursement: Pubkey,
    request_rent_payer: Option<Pubkey>,
) -> Instruction {
    let validator_compat = validator.compatize();
    let delegated_account_compat = delegated_account.compatize();
    let undelegate_buffer_pda =
        undelegate_buffer_pda_from_delegated_account(&delegated_account_compat)
            .modernize();
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
    let validator_fees_vault_pda =
        validator_fees_vault_pda_from_validator(&validator_compat).modernize();
    let mut accounts = vec![
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
    ];

    if let Some(request_rent_payer) = request_rent_payer {
        let undelegation_request_pda =
            undelegation_request_pda_from_delegated_account(
                &delegated_account_compat,
            )
            .modernize();
        accounts.push(AccountMeta::new(undelegation_request_pda, false));
        accounts.push(AccountMeta::new(request_rent_payer, false));
    }

    Instruction {
        program_id: dlp::id().modernize(),
        accounts,
        data: DlpDiscriminator::Undelegate.to_vec(),
    }
}

///
/// Returns accounts-data-size budget for undelegate instruction.
///
/// This value can be used with ComputeBudgetInstruction::SetLoadedAccountsDataSizeLimit
///
pub fn undelegate_size_budget(delegated_account: AccountSizeClass) -> u32 {
    total_size_budget(&[
        DLP_PROGRAM_DATA_SIZE_CLASS,
        AccountSizeClass::Tiny, // validator
        delegated_account,      // delegated_account
        AccountSizeClass::Tiny, // owner_program
        delegated_account,      // undelegate_buffer_pda
        delegated_account,      // commit_state_pda
        AccountSizeClass::Tiny, // commit_record_pda
        AccountSizeClass::Tiny, // delegation_record_pda
        AccountSizeClass::Tiny, // delegation_metadata_pda
        AccountSizeClass::Tiny, // rent_reimbursement
        AccountSizeClass::Tiny, // fees_vault_pda
        AccountSizeClass::Tiny, // validator_fees_vault_pda
        AccountSizeClass::Tiny, // system_program
    ])
}

///
/// Returns accounts-data-size budget for undelegate instruction with request
/// accounts.
///
/// This value can be used with ComputeBudgetInstruction::SetLoadedAccountsDataSizeLimit
///
pub fn undelegate_with_request_size_budget(
    delegated_account: AccountSizeClass,
) -> u32 {
    total_size_budget(&[
        DLP_PROGRAM_DATA_SIZE_CLASS,
        AccountSizeClass::Tiny, // validator
        delegated_account,      // delegated_account
        AccountSizeClass::Tiny, // owner_program
        delegated_account,      // undelegate_buffer_pda
        delegated_account,      // commit_state_pda
        AccountSizeClass::Tiny, // commit_record_pda
        AccountSizeClass::Tiny, // delegation_record_pda
        AccountSizeClass::Tiny, // delegation_metadata_pda
        AccountSizeClass::Tiny, // rent_reimbursement
        AccountSizeClass::Tiny, // fees_vault_pda
        AccountSizeClass::Tiny, // validator_fees_vault_pda
        AccountSizeClass::Tiny, // system_program
        AccountSizeClass::Tiny, // undelegation_request_pda
        AccountSizeClass::Tiny, // request_rent_payer
    ])
}
