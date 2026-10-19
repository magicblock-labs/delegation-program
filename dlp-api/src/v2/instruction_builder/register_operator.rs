use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;
use wheels::layout::Encodable;

use crate::{
    compat::Modernize,
    v2::{pda::operator_bond_pda, DlpV2Instruction, RegisterOperatorArgs},
};

/// Builds the instruction that registers one operator for v2 commitments.
pub fn register_operator(
    current_validator: Pubkey,
    config_authority: Pubkey,
    delegation_cluster: Pubkey,
    args: RegisterOperatorArgs,
) -> Instruction {
    Instruction {
        program_id: crate::id().modernize(),
        accounts: vec![
            AccountMeta::new(current_validator, true),
            AccountMeta::new_readonly(config_authority, true),
            AccountMeta::new(
                operator_bond_pda(&delegation_cluster).modernize(),
                false,
            ),
            AccountMeta::new_readonly(delegation_cluster, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: [
            DlpV2Instruction::RegisterOperator.to_vec(),
            args.encode().unwrap(),
        ]
        .concat(),
    }
}
