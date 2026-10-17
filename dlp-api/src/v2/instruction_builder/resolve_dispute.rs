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
            challenge_pda, operator_bond_pda, pending_commitment_pda,
            protocol_config_pda,
        },
        DlpV2Instruction, ResolveDisputeArgs,
    },
};

/// Builds the instruction that applies a resolver decision for a v2 challenge.
pub fn resolve_dispute(
    resolver: Pubkey,
    operator: Pubkey,
    challenger: Pubkey,
    account: Pubkey,
    commit_id: u64,
    args: ResolveDisputeArgs,
) -> Instruction {
    Instruction {
        program_id: crate::id().modernize(),
        accounts: vec![
            AccountMeta::new_readonly(resolver, true),
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
            AccountMeta::new(
                operator_bond_pda(&operator.compatize()).modernize(),
                false,
            ),
            AccountMeta::new(challenger, false),
            AccountMeta::new_readonly(protocol_config_pda().modernize(), false),
            AccountMeta::new(fees_vault_pda().modernize(), false),
        ],
        data: [
            DlpV2Instruction::ResolveDispute.to_vec(),
            args.encode().unwrap(),
        ]
        .concat(),
    }
}
