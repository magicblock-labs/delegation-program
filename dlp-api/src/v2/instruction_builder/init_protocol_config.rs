use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;

use crate::{
    compat::{borsh::to_vec, Modernize},
    v2::{
        pda::{protocol_config_pda, verifier_registry_pda},
        DlpV2Instruction, InitProtocolConfigArgs,
    },
};

/// Builds the instruction that creates the global v2 config accounts.
pub fn init_protocol_config(
    authority: Pubkey,
    args: InitProtocolConfigArgs,
) -> Instruction {
    Instruction {
        program_id: crate::id().modernize(),
        accounts: vec![
            AccountMeta::new(authority, true),
            AccountMeta::new(protocol_config_pda().modernize(), false),
            AccountMeta::new(verifier_registry_pda().modernize(), false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: [
            DlpV2Instruction::InitProtocolConfig.to_vec(),
            to_vec(&args).unwrap(),
        ]
        .concat(),
    }
}
