use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;

use crate::{
    compat::{Compatize, Modernize},
    pda::{
        delegation_metadata_pda_from_delegated_account,
        delegation_record_pda_from_delegated_account,
    },
    v2::{
        pda::{pending_commitment_pda, state_buffer_pda},
        DlpV2Instruction,
    },
};

/// Builds the instruction that finalizes one approved v2 commitment.
pub fn finalize_commitment(
    operator: Pubkey,
    account: Pubkey,
    commit_id: u64,
) -> Instruction {
    Instruction {
        program_id: crate::id().modernize(),
        accounts: vec![
            AccountMeta::new(operator, true),
            AccountMeta::new(
                pending_commitment_pda(&account.compatize(), commit_id)
                    .modernize(),
                false,
            ),
            AccountMeta::new(account, false),
            AccountMeta::new(
                delegation_record_pda_from_delegated_account(
                    &account.compatize(),
                )
                .modernize(),
                false,
            ),
            AccountMeta::new(
                delegation_metadata_pda_from_delegated_account(
                    &account.compatize(),
                )
                .modernize(),
                false,
            ),
            AccountMeta::new_readonly(
                state_buffer_pda(
                    &account.compatize(),
                    commit_id,
                    &operator.compatize(),
                )
                .modernize(),
                false,
            ),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: DlpV2Instruction::FinalizeCommitment.to_vec(),
    }
}
