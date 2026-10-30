use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use wheels::layout::Encodable;

use crate::{
    compat::{Compatize, Modernize},
    legacy::pda::fees_vault_pda,
    pda::{challenge_pda, operator_bond_pda, pending_commitment_pda},
    DlpV2Instruction, ResolveDisputeArgs,
};

/// Builds the instruction that applies a resolver decision for a v2 challenge.
pub fn resolve_dispute(
    resolver: Pubkey,
    delegation_cluster: Pubkey,
    challenger: Pubkey,
    account: Pubkey,
    commit_seq: u64,
    args: ResolveDisputeArgs,
) -> Instruction {
    Instruction {
        program_id: crate::id().modernize(),
        accounts: vec![
            AccountMeta::new_readonly(resolver, true),
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
            AccountMeta::new(
                operator_bond_pda(&delegation_cluster.compatize()).modernize(),
                false,
            ),
            AccountMeta::new(challenger, false),
            AccountMeta::new_readonly(delegation_cluster, false),
            AccountMeta::new(fees_vault_pda().modernize(), false),
        ],
        data: [
            DlpV2Instruction::ResolveDispute.to_vec(),
            args.encode().unwrap(),
        ]
        .concat(),
    }
}
