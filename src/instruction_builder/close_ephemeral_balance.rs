use solana_program::instruction::Instruction;
use solana_program::{instruction::AccountMeta, pubkey::Pubkey};

use crate::consts::EPHEMERAL_BALANCE;
use crate::discriminant::DlpDiscriminant;

pub fn close_ephemeral_balance(payer: Pubkey, index: u8) -> Instruction {
    let ephemeral_balance = Pubkey::find_program_address(
        &[EPHEMERAL_BALANCE, &payer.to_bytes(), &[index]],
        &crate::id(),
    )
    .0;
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(ephemeral_balance, false),
        ],
        data: [DlpDiscriminant::CloseEphemeralBalance.to_vec(), vec![index]].concat(),
    }
}
