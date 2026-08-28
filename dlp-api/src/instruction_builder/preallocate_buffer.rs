use dlp::{
    args::{PreallocateBufferArgs, PreallocateBufferKind},
    discriminator::DlpDiscriminator,
    pda::{
        commit_record_pda_from_delegated_account,
        commit_state_pda_from_delegated_account,
        delegation_record_pda_from_delegated_account,
        undelegate_buffer_pda_from_delegated_account,
        validator_fees_vault_pda_from_validator,
    },
    total_size_budget, AccountSizeClass, DLP_PROGRAM_DATA_SIZE_CLASS,
};
use solana_program::{
    account_info::MAX_PERMITTED_DATA_INCREASE,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;

use crate::compat::{borsh::to_vec, Compatize, Modernize};

/// Builds a preallocate buffer instruction.
/// See [dlp::processor::process_preallocate_buffer] for docs.
pub fn preallocate_buffer(
    validator: Pubkey,
    delegated_account: Pubkey,
    kind: PreallocateBufferKind,
    target_size: u32,
) -> Instruction {
    let args = to_vec(&PreallocateBufferArgs { kind, target_size }).unwrap();
    let validator_compat = validator.compatize();
    let delegated_account_compat = delegated_account.compatize();
    let delegation_record_pda =
        delegation_record_pda_from_delegated_account(&delegated_account_compat)
            .modernize();
    let commit_record_pda =
        commit_record_pda_from_delegated_account(&delegated_account_compat)
            .modernize();
    let validator_fees_vault_pda =
        validator_fees_vault_pda_from_validator(&validator_compat).modernize();
    let buffer_pda = match kind {
        PreallocateBufferKind::CommitState => {
            commit_state_pda_from_delegated_account(&delegated_account_compat)
                .modernize()
        }
        PreallocateBufferKind::UndelegateBuffer => {
            undelegate_buffer_pda_from_delegated_account(
                &delegated_account_compat,
            )
            .modernize()
        }
        PreallocateBufferKind::DelegatedAccount => delegated_account,
    };
    Instruction {
        program_id: dlp::id().modernize(),
        accounts: vec![
            AccountMeta::new(validator, true),
            AccountMeta::new_readonly(delegated_account, false),
            AccountMeta::new(delegation_record_pda, false),
            AccountMeta::new(buffer_pda, false),
            AccountMeta::new_readonly(commit_record_pda, false),
            AccountMeta::new_readonly(validator_fees_vault_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: [DlpDiscriminator::PreallocateBuffer.to_vec(), args].concat(),
    }
}

/// Returns the sequence of `preallocate_buffer` instructions needed to grow
/// a buffer of `kind` from `current_size` up to `target_size`
pub fn preallocate_buffer_chunks(
    validator: Pubkey,
    delegated_account: Pubkey,
    kind: PreallocateBufferKind,
    current_size: u32,
    target_size: u32,
) -> Vec<Instruction> {
    let growth = target_size.saturating_sub(current_size);
    let chunks = growth.div_ceil(MAX_PERMITTED_DATA_INCREASE as u32) as usize;
    (0..chunks)
        .map(|_| {
            preallocate_buffer(validator, delegated_account, kind, target_size)
        })
        .collect()
}

///
/// Returns accounts-data-size budget for preallocate_buffer instruction.
///
/// This value can be used with ComputeBudgetInstruction::SetLoadedAccountsDataSizeLimit
///
pub fn preallocate_buffer_size_budget(
    delegated_account: AccountSizeClass,
) -> u32 {
    total_size_budget(&[
        DLP_PROGRAM_DATA_SIZE_CLASS,
        AccountSizeClass::Tiny, // validator
        delegated_account,      // delegated_account
        AccountSizeClass::Tiny, // delegation_record_pda
        delegated_account,      // buffer_pda
        AccountSizeClass::Tiny, // commit_record_pda
        AccountSizeClass::Tiny, // validator_fees_vault_pda
        AccountSizeClass::Tiny, // system_program
    ])
}
