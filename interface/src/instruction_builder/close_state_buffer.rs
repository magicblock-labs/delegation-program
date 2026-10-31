use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use wheels::{layout::Encodable, DataLayoutError};

use crate::{
    compat::{Compatize, Modernize},
    pda::{pending_commitment_pda_with_bump, state_buffer_pda_with_bump},
    CloseStateBufferArgs, DlpV2Instruction,
};

/// Builds an instruction that closes one StateBuffer.
pub fn close_state_buffer(
    authority: Pubkey,
    account: Pubkey,
    commit_seq: u32,
    rent_payer: Pubkey,
) -> Result<Instruction, DataLayoutError> {
    let account = account.compatize();
    let authority = authority.compatize();
    let (state_buffer, state_buffer_bump) =
        state_buffer_pda_with_bump(&account, commit_seq, &authority);
    let (pending_commitment, pending_commitment_bump) =
        pending_commitment_pda_with_bump(&account, commit_seq);

    Ok(Instruction {
        program_id: crate::id().modernize(),
        accounts: vec![
            AccountMeta::new_readonly(authority.modernize(), true),
            AccountMeta::new(state_buffer.modernize(), false),
            AccountMeta::new_readonly(account.modernize(), false),
            AccountMeta::new(rent_payer, false),
            AccountMeta::new_readonly(pending_commitment.modernize(), false),
        ],
        data: [
            DlpV2Instruction::CloseStateBuffer.to_vec(),
            CloseStateBufferArgs {
                commit_seq,
                state_buffer_bump,
                pending_commitment_bump,
            }
            .encode()?,
        ]
        .concat(),
    })
}
