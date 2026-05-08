use dlp::{
    args::DelegateEphemeralBalanceArgs,
    discriminator::DlpDiscriminator,
    pda::{
        delegate_buffer_pda_from_delegated_account_and_owner_program,
        delegation_metadata_pda_from_delegated_account,
        delegation_record_pda_from_delegated_account,
        ephemeral_balance_pda_from_payer,
    },
};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;

use crate::compat::{borsh::to_vec, Compatize, Modernize};

/// Delegate ephemeral balance
/// See [dlp::processor::process_delegate_ephemeral_balance] for docs.
pub fn delegate_ephemeral_balance(
    payer: Pubkey,
    pubkey: Pubkey,
    args: DelegateEphemeralBalanceArgs,
) -> Instruction {
    let pubkey_compat = pubkey.compatize();
    let delegated_account =
        ephemeral_balance_pda_from_payer(&pubkey_compat, args.index);
    let system_program_compat = system_program::id().compatize();
    let delegate_buffer_pda =
        delegate_buffer_pda_from_delegated_account_and_owner_program(
            &delegated_account,
            &system_program_compat,
        );
    let delegation_record_pda =
        delegation_record_pda_from_delegated_account(&delegated_account);
    let delegation_metadata_pda =
        delegation_metadata_pda_from_delegated_account(&delegated_account);
    let delegated_account = delegated_account.modernize();
    let mut data = DlpDiscriminator::DelegateEphemeralBalance.to_vec();
    data.extend_from_slice(&to_vec(&args).unwrap());

    Instruction {
        program_id: dlp::id().modernize(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(pubkey, true),
            AccountMeta::new(delegated_account, false),
            AccountMeta::new(delegate_buffer_pda.modernize(), false),
            AccountMeta::new(delegation_record_pda.modernize(), false),
            AccountMeta::new(delegation_metadata_pda.modernize(), false),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(dlp::id().modernize(), false),
        ],
        data,
    }
}
