use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;
use wheels::layout::Encodable;

use crate::{
    compat::{Compatize, Modernize},
    v2::{
        pda::{protocol_config_pda, state_buffer_pda},
        DlpV2Instruction, WriteStateBufferArgs,
    },
};

/// Builds the instruction that writes full account-state bytes to a v2 buffer.
pub fn write_state_buffer(
    payer: Pubkey,
    authority: Pubkey,
    account: Pubkey,
    args: WriteStateBufferArgs,
) -> Instruction {
    Instruction {
        program_id: crate::id().modernize(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(authority, true),
            AccountMeta::new(
                state_buffer_pda(
                    &account.compatize(),
                    args.commit_id,
                    &authority.compatize(),
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
