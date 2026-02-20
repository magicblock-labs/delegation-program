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

pub const MAX_CU_COMMIT_FINALIZE_FROM_BUFFER: u32 = 2000;

/// Builds a commit state from buffer instruction.
/// See [crate::processor::process_commit_diff_from_buffer] for docs.
pub fn v2_commit_finalize_from_buffer(
    validator: Pubkey,
    delegated_account: Pubkey,
    data_buffer: Pubkey,
    commit_args: &mut CommitFinalizeArgs,
) -> Instruction {
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
            AccountMeta::new_readonly(data_buffer, false),
            //        AccountMeta::new_readonly(validator_fees_vault, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: [
            DlpInstruction::CommitFinalizeFromBuffer.to_vec(),
            commit_args.to_bytes(),
        ]
        .concat(),
    }
}

///
/// Returns accounts-data-size budget for commit_diff_from_buffer instruction.
///
/// This value can be used with ComputeBudgetInstruction::SetLoadedAccountsDataSizeLimit
///
pub fn v2_commit_finalize_from_buffer_size_budget(
    delegated_account: AccountSizeClass,
) -> u32 {
    total_size_budget(&[
        DLP_PROGRAM_DATA_SIZE_CLASS,
        AccountSizeClass::Tiny, // validator
        delegated_account,      // delegated_account
        AccountSizeClass::Tiny, // delegation_state
        delegated_account,      // data_buffer
        AccountSizeClass::Tiny, // validator_fees_vault_pda
        AccountSizeClass::Tiny, // system_program
    ])
}
