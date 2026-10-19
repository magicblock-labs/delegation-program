use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;
use wheels::layout::Encodable;

use crate::{
    compat::Modernize,
    v2::{
        pda::{verifier_bond_pda, verifier_registry_pda},
        DlpV2Instruction, UpdateVerifierRegistryArgs,
    },
};

/// Builds the instruction that updates the verifier selection registry.
pub fn update_verifier_registry(
    config_authority: Pubkey,
    delegation_cluster: Pubkey,
    verifier: Pubkey,
    args: UpdateVerifierRegistryArgs,
) -> Instruction {
    Instruction {
        program_id: crate::id().modernize(),
        accounts: vec![
            AccountMeta::new(config_authority, true),
            AccountMeta::new_readonly(delegation_cluster, false),
            AccountMeta::new(
                verifier_registry_pda(&delegation_cluster).modernize(),
                false,
            ),
            AccountMeta::new_readonly(
                verifier_bond_pda(&delegation_cluster, &verifier).modernize(),
                false,
            ),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: [
            DlpV2Instruction::UpdateVerifierRegistry.to_vec(),
            args.encode().unwrap(),
        ]
        .concat(),
    }
}
