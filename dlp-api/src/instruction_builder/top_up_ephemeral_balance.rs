use dlp::{
    args::TopUpEphemeralBalanceArgs, discriminator::DlpDiscriminator,
    pda::ephemeral_balance_pda_from_payer,
};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;

use crate::compat::{borsh::to_vec, Compatize, Modernize};

/// Builds a top-up ephemeral balance instruction.
/// See [dlp::processor::process_top_up_ephemeral_balance] for docs.
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
    let pubkey_compat = pubkey.compatize();
    let ephemeral_balance_pda =
        ephemeral_balance_pda_from_payer(&pubkey_compat, args.index)
            .modernize();
    Instruction {
        program_id: dlp::id().modernize(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(pubkey, false),
            AccountMeta::new(ephemeral_balance_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: [
            DlpDiscriminator::TopUpEphemeralBalance.to_vec(),
            to_vec(&args).unwrap(),
        ]
        .concat(),
    }
}
