use solana_program::instruction::Instruction;
use solana_program::{instruction::AccountMeta, pubkey::Pubkey};

use crate::discriminant::DlpDiscriminant;
use crate::pda::ephemeral_balance_pda_from_payer;

pub fn close_ephemeral_balance(payer: Pubkey, index: u8) -> Instruction {
    let ephemeral_balance = ephemeral_balance_pda_from_payer(&payer, index);
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(ephemeral_balance, false),
        ],
        data: [DlpDiscriminant::CloseEphemeralBalance.to_vec(), vec![index]].concat(),
    }
}
