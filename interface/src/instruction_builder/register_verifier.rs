use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;
use wheels::layout::Encodable;

use crate::{
    compat::{Compatize, Modernize},
    pda::verifier_bond_pda,
    DlpV2Instruction, RegisterVerifierArgs,
};

/// Builds the instruction that registers one verifier for v2 approvals.
pub fn register_verifier(
    verifier: Pubkey,
    config_authority: Pubkey,
    delegation_cluster: Pubkey,
    args: RegisterVerifierArgs,
) -> Instruction {
    Instruction {
        program_id: crate::id().modernize(),
        accounts: vec![
            AccountMeta::new(verifier, true),
            AccountMeta::new_readonly(config_authority, true),
            AccountMeta::new(
                verifier_bond_pda(
                    &delegation_cluster.compatize(),
                    &verifier.compatize(),
                )
                .modernize(),
                false,
            ),
            AccountMeta::new_readonly(delegation_cluster, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: [
            DlpV2Instruction::RegisterVerifier.to_vec(),
            args.encode().unwrap(),
        ]
        .concat(),
    }
}
