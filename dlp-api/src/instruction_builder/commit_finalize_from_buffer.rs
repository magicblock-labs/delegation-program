use dlp::{
    args::{CommitBumps, CommitFinalizeArgs},
    delegation_metadata_seeds_from_delegated_account,
    delegation_record_seeds_from_delegated_account,
    discriminator::DlpDiscriminator,
    pod_view::PodView,
    total_size_budget, validator_fees_vault_seeds_from_validator,
    AccountSizeClass, DLP_PROGRAM_DATA_SIZE_CLASS,
};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    system_program,
};

/// Builds a commit state from buffer instruction.
/// See [dlp::processor::process_commit_diff_from_buffer] for docs.
pub fn commit_finalize_from_buffer(
    validator: Pubkey,
    delegated_account: Pubkey,
    data_buffer: Pubkey,
    commit_args: &mut CommitFinalizeArgs,
) -> (Instruction, super::CommitPDAs) {
    let delegation_record = Pubkey::find_program_address(
        delegation_record_seeds_from_delegated_account!(delegated_account),
        &dlp::id(),
    );

    let validator_fees_vault = Pubkey::find_program_address(
        validator_fees_vault_seeds_from_validator!(validator),
        &dlp::id(),
    );

    let delegation_metadata = Pubkey::find_program_address(
        delegation_metadata_seeds_from_delegated_account!(delegated_account),
        &dlp::id(),
    );

    // save the bumps in the args
    commit_args.bumps = CommitBumps {
        delegation_record: delegation_record.1,
        delegation_metadata: delegation_metadata.1,
        validator_fees_vault: validator_fees_vault.1,
    };

    (
        Instruction {
            program_id: dlp::id(),
            accounts: vec![
                AccountMeta::new(validator, true),
                AccountMeta::new(delegated_account, false),
                AccountMeta::new(delegation_record.0, false),
                AccountMeta::new(delegation_metadata.0, false),
                AccountMeta::new_readonly(data_buffer, false),
                AccountMeta::new(validator_fees_vault.0, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: [
                DlpDiscriminator::CommitFinalizeFromBuffer.to_vec(),
                commit_args.to_bytes(),
            ]
            .concat(),
        },
        super::CommitPDAs {
            delegation_record: delegation_record.0,
            delegation_metadata: delegation_metadata.0,
            validator_fees_vault: validator_fees_vault.0,
        },
    )
}

///
/// Returns accounts-data-size budget for commit_diff_from_buffer instruction.
///
/// This value can be used with ComputeBudgetInstruction::SetLoadedAccountsDataSizeLimit
///
pub fn commit_finalize_from_buffer_size_budget(
    delegated_account: AccountSizeClass,
) -> u32 {
    total_size_budget(&[
        DLP_PROGRAM_DATA_SIZE_CLASS,
        AccountSizeClass::Tiny, // validator
        delegated_account,      // delegated_account
        AccountSizeClass::Tiny, // delegation_record_pda
        AccountSizeClass::Tiny, // delegation_metadata_pda
        delegated_account,      // data_buffer
        AccountSizeClass::Tiny, // validator_fees_vault_pda
        AccountSizeClass::Tiny, // program_config_pda
        AccountSizeClass::Tiny, // system_program
    ])
}
