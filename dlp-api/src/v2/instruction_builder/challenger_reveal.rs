use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use wheels::layout::Encodable;

use crate::{
    compat::{Compatize, Modernize},
    pda::fees_vault_pda,
    v2::{
        pda::{
            challenge_pda, pending_commitment_pda, protocol_config_pda,
            state_buffer_pda,
        },
        ChallengerRevealArgs, DlpV2Instruction,
    },
};

/// Builds the instruction that reveals challenger state for a v2 challenge.
pub fn challenger_reveal(
    challenger: Pubkey,
    operator: Pubkey,
    account: Pubkey,
    commit_id: u64,
    args: ChallengerRevealArgs,
) -> Instruction {
    Instruction {
        program_id: crate::id().modernize(),
        accounts: vec![
            AccountMeta::new(challenger, true),
            AccountMeta::new(
                challenge_pda(
                    &account.compatize(),
                    commit_id,
                    &challenger.compatize(),
                )
                .modernize(),
                false,
            ),
            AccountMeta::new(
                pending_commitment_pda(&account.compatize(), commit_id)
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
            AccountMeta::new_readonly(
                state_buffer_pda(
                    &account.compatize(),
                    commit_id,
                    &challenger.compatize(),
                )
                .modernize(),
                false,
            ),
            AccountMeta::new_readonly(protocol_config_pda().modernize(), false),
            AccountMeta::new(fees_vault_pda().modernize(), false),
        ],
        data: [
            DlpV2Instruction::ChallengerReveal.to_vec(),
            args.encode().unwrap(),
        ]
        .concat(),
    }
}
