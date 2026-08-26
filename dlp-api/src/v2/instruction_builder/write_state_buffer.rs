use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;
use wheels::layout::Encodable;

use crate::{
    compat::{Compatize, Modernize},
    v2::{
        pda::{operator_bond_pda, protocol_config_pda, state_buffer_pda},
        DlpV2Instruction, WriteStateBufferArgs,
    },
};

/// Builds the instruction that writes full account-state bytes to a v2 buffer.
pub fn write_state_buffer(
    payer: Pubkey,
    operator: Pubkey,
    account: Pubkey,
    args: WriteStateBufferArgs,
) -> Instruction {
    Instruction {
        program_id: crate::id().modernize(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(operator, true),
            AccountMeta::new_readonly(
                operator_bond_pda(&operator.compatize()).modernize(),
                false,
            ),
            AccountMeta::new(
                state_buffer_pda(
                    &account.compatize(),
                    args.commit_id,
                    &operator.compatize(),
                )
                .modernize(),
                false,
            ),
            AccountMeta::new_readonly(account, false),
            AccountMeta::new_readonly(protocol_config_pda().modernize(), false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: [
            DlpV2Instruction::WriteStateBuffer.to_vec(),
            args.encode().unwrap(),
        ]
        .concat(),
    }
}
