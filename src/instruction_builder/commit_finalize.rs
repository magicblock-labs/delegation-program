use solana_program::instruction::Instruction;
use solana_program::system_program;
use solana_program::{instruction::AccountMeta, pubkey::Pubkey};

use crate::args::CommitFinalizeArgs;
use crate::discriminator::DlpDiscriminator;
use crate::pod_view::PodView;
use crate::{
    delegation_metadata_seeds_from_delegated_account,
    delegation_record_seeds_from_delegated_account, program_config_seeds_from_program_id,
    total_size_budget, validator_fees_vault_seeds_from_validator, AccountSizeClass,
    DLP_PROGRAM_DATA_SIZE_CLASS,
};

/// Builds a commit finalize instruction.
/// See [crate::processor::process_commit_finalize] for docs.
pub fn commit_finalize(
    validator: Pubkey,
    delegated_account: Pubkey,
    delegated_account_owner: Pubkey,
    commit_args: &mut CommitFinalizeArgs,
    data: &[u8],
) -> Instruction {
    let delegation_record = Pubkey::find_program_address(
        delegation_record_seeds_from_delegated_account!(delegated_account),
        &crate::id(),
    );

    let validator_fees_vault = Pubkey::find_program_address(
        validator_fees_vault_seeds_from_validator!(validator),
        &crate::id(),
    );

    let delegation_metadata = Pubkey::find_program_address(
        delegation_metadata_seeds_from_delegated_account!(delegated_account),
        &crate::id(),
    );

    let program_config = Pubkey::find_program_address(
        program_config_seeds_from_program_id!(delegated_account_owner),
        &crate::id(),
    );

    // save the bumps in the args
    commit_args.delegation_record_bump = delegation_record.1;
    commit_args.delegation_metadata_bump = delegation_metadata.1;
    commit_args.validator_fees_vault_bump = validator_fees_vault.1;
    commit_args.program_config_bump = program_config.1;

    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new_readonly(validator, true),
            AccountMeta::new(delegated_account, false),
            AccountMeta::new_readonly(delegation_record.0, false),
            AccountMeta::new(delegation_metadata.0, false),
            AccountMeta::new_readonly(validator_fees_vault.0, false),
            AccountMeta::new_readonly(program_config.0, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: [
            DlpDiscriminator::CommitFinalize.to_vec(),
            commit_args.to_bytes(),
            data.to_vec(),
        ]
        .concat(),
    }
}

///
/// Returns accounts-data-size budget for commit_state instruction.
///
/// This value can be used with ComputeBudgetInstruction::SetLoadedAccountsDataSizeLimit
///
pub fn commit_finalize_size_budget(delegated_account: AccountSizeClass) -> u32 {
    total_size_budget(&[
        DLP_PROGRAM_DATA_SIZE_CLASS,
        AccountSizeClass::Tiny, // validator
        delegated_account,      // delegated_account
        AccountSizeClass::Tiny, // delegation_record_pda
        AccountSizeClass::Tiny, // delegation_metadata_pda
        AccountSizeClass::Tiny, // validator_fees_vault_pda
        AccountSizeClass::Tiny, // program_config_pda
        AccountSizeClass::Tiny, // system_program
    ])
}
