use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    system_program,
};

use crate::{
    pod_view::PodView,
    total_size_budget,
    v2::{CommitFinalizeArgs, DelegationStateHeader, DlpInstruction},
    AccountSizeClass, DLP_PROGRAM_DATA_SIZE_CLASS,
};

pub const MAX_CU_COMMIT_FINALIZE: u32 = 2000;

/// Builds a commit finalize instruction.
/// See [crate::processor::process_commit_finalize] for docs.
pub fn v2_commit_finalize(
    validator: Pubkey,
    delegated_account: Pubkey,
    args: &mut CommitFinalizeArgs,
    state_or_diff: &[u8],
) -> Instruction {
    println!("v2_commit_finalize");
    let delegation_state = Pubkey::find_program_address(
        &[DelegationStateHeader::SEED, delegated_account.as_ref()],
        &crate::id(),
    )
    .0;

    // let validator_fees_vault = Pubkey::find_program_address(
    //     validator_fees_vault_seeds_from_validator!(validator),
    //     &crate::id(),
    // )
    // .0;

    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new_readonly(validator, true),
            AccountMeta::new(delegated_account, false),
            AccountMeta::new(delegation_state, false),
            //        AccountMeta::new_readonly(validator_fees_vault, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: [
            DlpInstruction::CommitFinalize.to_vec(),
            args.to_bytes(),
            state_or_diff.to_vec(),
        ]
        .concat(),
    }
}

pub fn v2_commit_finalize_inline(
    validator: Pubkey,
    delegated_account: Pubkey,
    args: &mut CommitFinalizeArgs,
    state_or_diff: &[u8],
) -> Instruction {
    println!("v2_commit_finalize_ugly");
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new_readonly(validator, true),
            AccountMeta::new(delegated_account, false),
            // AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: [
            DlpInstruction::CommitFinalizeInline.to_vec(),
            args.to_bytes(),
            state_or_diff.to_vec(),
        ]
        .concat(),
    }
}

///
/// Returns accounts-data-size budget for commit_finalize instruction.
///
/// This value can be used with ComputeBudgetInstruction::SetLoadedAccountsDataSizeLimit
///
pub fn v2_commit_finalize_size_budget(
    delegated_account: AccountSizeClass,
) -> u32 {
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
