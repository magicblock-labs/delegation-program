use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;
use wheels::layout::Encodable;

use crate::{
    compat::{Compatize, Modernize},
    v2::{
        pda::{challenge_pda, pending_commitment_pda, protocol_config_pda},
        DlpV2Instruction, RaiseChallengeArgs,
    },
};

/// Builds the instruction that raises a hash-only v2 challenge.
pub fn raise_challenge(
    challenger: Pubkey,
    account: Pubkey,
    commit_id: u64,
    args: RaiseChallengeArgs,
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
            AccountMeta::new_readonly(protocol_config_pda().modernize(), false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: [
            DlpV2Instruction::RaiseChallenge.to_vec(),
            args.encode().unwrap(),
        ]
        .concat(),
    }
}
