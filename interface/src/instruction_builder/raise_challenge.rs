use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;
use wheels::layout::Encodable;

use crate::{
    compat::{Compatize, Modernize},
    pda::{challenge_pda, pending_commitment_pda},
    DlpV2Instruction, RaiseChallengeArgs,
};

/// Builds the instruction that raises a hash-only v2 challenge.
pub fn raise_challenge(
    challenger: Pubkey,
    delegation_cluster: Pubkey,
    account: Pubkey,
    commit_seq: u32,
    args: RaiseChallengeArgs,
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
            AccountMeta::new_readonly(delegation_cluster, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: [
            DlpV2Instruction::RaiseChallenge.to_vec(),
            args.encode().unwrap(),
        ]
        .concat(),
    }
}
