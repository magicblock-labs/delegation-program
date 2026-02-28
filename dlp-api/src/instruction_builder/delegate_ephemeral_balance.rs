use borsh::to_vec;
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
    system_program,
};

/// Delegate ephemeral balance
/// See [dlp::processor::process_delegate_ephemeral_balance] for docs.
pub fn delegate_ephemeral_balance(
    payer: Pubkey,
    pubkey: Pubkey,
    args: DelegateEphemeralBalanceArgs,
) -> Instruction {
    let delegated_account =
        ephemeral_balance_pda_from_payer(&pubkey, args.index);
    let delegate_buffer_pda =
        delegate_buffer_pda_from_delegated_account_and_owner_program(
            &delegated_account,
            &system_program::id(),
        );
    let delegation_record_pda =
        delegation_record_pda_from_delegated_account(&delegated_account);
    let delegation_metadata_pda =
        delegation_metadata_pda_from_delegated_account(&delegated_account);
    let mut data = DlpDiscriminator::DelegateEphemeralBalance.to_vec();
    data.extend_from_slice(&to_vec(&args).unwrap());

    Instruction {
        program_id: dlp::id(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(pubkey, true),
            AccountMeta::new(delegated_account, false),
            AccountMeta::new(delegate_buffer_pda, false),
            AccountMeta::new(delegation_record_pda, false),
            AccountMeta::new(delegation_metadata_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(dlp::id(), false),
        ],
        data,
    }
}
