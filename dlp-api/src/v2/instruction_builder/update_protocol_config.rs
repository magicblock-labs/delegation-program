use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use wheels::layout::Encodable;

use crate::{
    compat::Modernize,
    v2::{
        pda::protocol_config_pda, DlpV2Instruction, UpdateProtocolConfigArgs,
    },
};

/// Builds the instruction that updates global v2 config for future work.
pub fn update_protocol_config(
    authority: Pubkey,
    args: UpdateProtocolConfigArgs,
) -> Instruction {
    Instruction {
        program_id: crate::id().modernize(),
        accounts: vec![
            AccountMeta::new_readonly(authority, true),
            AccountMeta::new(protocol_config_pda().modernize(), false),
        ],
        data: [
            DlpV2Instruction::UpdateProtocolConfig.to_vec(),
            args.encode().unwrap(),
        ]
        .concat(),
    }
}
