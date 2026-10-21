use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;
use wheels::{layout::Encodable, DataLayoutError};

use crate::{
    compat::{Compatize, Modernize},
    pda::{delegation_state_pda, state_buffer_pda},
    CommitFinalizeFromBufferArgs, DlpV2Instruction, PrepareCommitFinalizeArgs,
};

/// Builds a direct-cluster commit that consumes a finalized StateBuffer.
pub fn commit_finalize_from_buffer(
    current_validator: Pubkey,
    delegation_cluster: Pubkey,
    delegated_account: Pubkey,
    args: CommitFinalizeFromBufferArgs,
) -> Result<Instruction, DataLayoutError> {
    Ok(Instruction {
        program_id: crate::id().modernize(),
        accounts: vec![
            AccountMeta::new(current_validator, true),
            AccountMeta::new(delegated_account, false),
            AccountMeta::new(
                delegation_state_pda(&delegated_account.compatize())
                    .modernize(),
                false,
            ),
            AccountMeta::new_readonly(
                state_buffer_pda(
                    &delegated_account.compatize(),
                    args.commit_seq,
                    &current_validator.compatize(),
                )
                .modernize(),
                false,
            ),
            AccountMeta::new_readonly(delegation_cluster, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: [
            DlpV2Instruction::CommitFinalizeFromBuffer.to_vec(),
            args.encode()?,
        ]
        .concat(),
    })
}

/// Builds one growth step to run before finalizing a large account update.
pub fn prepare_commit_finalize(
    current_validator: Pubkey,
    delegation_cluster: Pubkey,
    delegated_account: Pubkey,
    args: PrepareCommitFinalizeArgs,
) -> Result<Instruction, DataLayoutError> {
    Ok(Instruction {
        program_id: crate::id().modernize(),
        accounts: vec![
            AccountMeta::new(current_validator, true),
            AccountMeta::new(delegated_account, false),
            AccountMeta::new_readonly(
                delegation_state_pda(&delegated_account.compatize())
                    .modernize(),
                false,
            ),
            AccountMeta::new_readonly(delegation_cluster, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: [
            DlpV2Instruction::PrepareCommitFinalize.to_vec(),
            args.encode()?,
        ]
        .concat(),
    })
}

/// Builds every growth step needed before finalizing `target_data_len`.
pub fn prepare_commit_finalize_instructions(
    current_validator: Pubkey,
    delegation_cluster: Pubkey,
    delegated_account: Pubkey,
    commit_seq: u64,
    current_data_len: usize,
    target_data_len: u32,
) -> Result<Vec<Instruction>, DataLayoutError> {
    let growth = (target_data_len as usize).saturating_sub(current_data_len);
    let step_count =
        growth.div_ceil(crate::StateBuffer::MAX_ACCOUNT_DATA_GROWTH_PER_WRITE);
    let mut instructions = Vec::with_capacity(step_count);

    for _ in 0..step_count {
        instructions.push(prepare_commit_finalize(
            current_validator,
            delegation_cluster,
            delegated_account,
            PrepareCommitFinalizeArgs {
                commit_seq,
                target_data_len,
            },
        )?);
    }

    Ok(instructions)
}
