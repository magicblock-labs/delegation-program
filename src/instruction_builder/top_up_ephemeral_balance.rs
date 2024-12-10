use borsh::BorshSerialize;
use solana_program::instruction::Instruction;
use solana_program::system_program;
use solana_program::{instruction::AccountMeta, pubkey::Pubkey};

use crate::args::TopUpEphemeralBalanceArgs;
use crate::discriminator::DlpDiscriminator;
use crate::pda::ephemeral_balance_pda_from_payer;

/// Builds a top-up ephemeral balance instruction.
pub fn top_up_ephemeral_balance(
    payer: Pubkey,
    pubkey: Pubkey,
    amount: Option<u64>,
    index: Option<u8>,
) -> Instruction {
    let args = TopUpEphemeralBalanceArgs {
        amount: amount.unwrap_or(10000),
        index: index.unwrap_or(0),
    };
    let ephemeral_balance_pda = ephemeral_balance_pda_from_payer(&pubkey, args.index);
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(pubkey, false),
            AccountMeta::new(ephemeral_balance_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: [
            DlpDiscriminator::TopUpEphemeralBalance.to_vec(),
            args.try_to_vec().unwrap(),
        ]
        .concat(),
    }
}
