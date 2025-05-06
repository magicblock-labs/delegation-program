use solana_program::instruction::Instruction;
use solana_program::{instruction::AccountMeta, pubkey::Pubkey, system_program};

use crate::discriminator::DlpDiscriminator;
use crate::pda::ephemeral_balance_pda_from_payer;

/// Creates instruction to close an ephemeral balance account
/// See [crate::processor::process_close_ephemeral_balance_v1] for docs.
/// [crate::processor::process_close_ephemeral_balance] now deprecated
pub fn close_ephemeral_balance(payer: Pubkey, index: u8) -> Instruction {
    let ephemeral_balance_pda = ephemeral_balance_pda_from_payer(&payer, index);
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(ephemeral_balance_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: [
            DlpDiscriminator::CloseEphemeralBalanceV1.to_vec(),
            vec![index],
        ]
        .concat(),
    }
}
