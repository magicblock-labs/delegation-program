use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use wheels::layout::Encodable;

use crate::{
    compat::{Compatize, Modernize},
    legacy::pda::fees_vault_pda,
    pda::{challenge_pda, pending_commitment_pda, state_buffer_pda},
    ChallengerRevealArgs, DlpV2Instruction,
};

/// Builds the instruction that reveals challenger state for a v2 challenge.
pub fn challenger_reveal(
    challenger: Pubkey,
    operator: Pubkey,
    delegation_cluster: Pubkey,
    account: Pubkey,
    commit_seq: u64,
    args: ChallengerRevealArgs,
) -> Instruction {
    Instruction {
        program_id: crate::id().modernize(),
        accounts: vec![
            AccountMeta::new(challenger, true),
            AccountMeta::new(
                challenge_pda(
                    &account.compatize(),
                    commit_seq,
                    &challenger.compatize(),
                )
                .modernize(),
                false,
            ),
            AccountMeta::new(
                pending_commitment_pda(&account.compatize(), commit_seq)
                    .modernize(),
                false,
            ),
            AccountMeta::new_readonly(
                state_buffer_pda(
                    &account.compatize(),
                    commit_seq,
                    &operator.compatize(),
                )
                .modernize(),
                false,
            ),
            AccountMeta::new_readonly(
                state_buffer_pda(
                    &account.compatize(),
                    commit_seq,
                    &challenger.compatize(),
                )
                .modernize(),
                false,
            ),
            AccountMeta::new_readonly(delegation_cluster, false),
            AccountMeta::new(fees_vault_pda().modernize(), false),
        ],
        data: [
            DlpV2Instruction::ChallengerReveal.to_vec(),
            args.encode().unwrap(),
        ]
        .concat(),
    }
}
