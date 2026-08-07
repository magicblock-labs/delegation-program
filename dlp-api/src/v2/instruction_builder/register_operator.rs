use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;
use wheels::layout::Encodable;

use crate::{
    compat::{Compatize, Modernize},
    v2::{
        pda::{operator_bond_pda, protocol_config_pda},
        DlpV2Instruction, RegisterOperatorArgs,
    },
};

/// Builds the instruction that registers one operator for v2 commitments.
pub fn register_operator(
    operator: Pubkey,
    authority: Pubkey,
    args: RegisterOperatorArgs,
) -> Instruction {
    Instruction {
        program_id: crate::id().modernize(),
        accounts: vec![
            AccountMeta::new(operator, true),
            AccountMeta::new_readonly(authority, true),
            AccountMeta::new(
                operator_bond_pda(&operator.compatize()).modernize(),
                false,
            ),
            AccountMeta::new_readonly(protocol_config_pda().modernize(), false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: [
            DlpV2Instruction::RegisterOperator.to_vec(),
            args.encode().unwrap(),
        ]
        .concat(),
    }
}
