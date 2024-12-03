use borsh::BorshSerialize;
use solana_program::instruction::Instruction;
use solana_program::system_program;
use solana_program::{instruction::AccountMeta, pubkey::Pubkey};

use crate::args::TopUpArgs;
use crate::consts::EPHEMERAL_BALANCE;
use crate::discriminant::DlpDiscriminant;

/// Builds a top-up ephemeral balance instruction.
pub fn top_up(payer: Pubkey, amount: Option<u64>, index: Option<u8>) -> Instruction {
    let args = TopUpArgs {
        amount: amount.unwrap_or(10000),
        index: index.unwrap_or(0),
    };
    let ephemeral_balance = Pubkey::find_program_address(
        &[EPHEMERAL_BALANCE, &payer.to_bytes(), &[args.index]],
        &crate::id(),
    )
    .0;
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(ephemeral_balance, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: [DlpDiscriminant::TopUp.to_vec(), args.try_to_vec().unwrap()].concat(),
    }
}
