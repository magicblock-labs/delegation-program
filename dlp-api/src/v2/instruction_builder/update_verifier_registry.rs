use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;
use wheels::layout::Encodable;

use crate::{
    compat::{Compatize, Modernize},
    v2::{
        pda::{protocol_config_pda, verifier_bond_pda, verifier_registry_pda},
        DlpV2Instruction, UpdateVerifierRegistryArgs,
    },
};

/// Builds the instruction that updates the verifier selection registry.
pub fn update_verifier_registry(
    authority: Pubkey,
    verifier: Pubkey,
    args: UpdateVerifierRegistryArgs,
) -> Instruction {
    Instruction {
        program_id: crate::id().modernize(),
        accounts: vec![
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(protocol_config_pda().modernize(), false),
            AccountMeta::new(verifier_registry_pda().modernize(), false),
            AccountMeta::new_readonly(
                verifier_bond_pda(&verifier.compatize()).modernize(),
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
