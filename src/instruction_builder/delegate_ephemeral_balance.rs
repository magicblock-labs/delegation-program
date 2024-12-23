use borsh::BorshSerialize;
use solana_program::instruction::Instruction;
use solana_program::system_program;
use solana_program::{instruction::AccountMeta, pubkey::Pubkey};

use crate::args::DelegateEphemeralBalanceArgs;
use crate::discriminator::DlpDiscriminator;
use crate::pda::{
    delegate_buffer_pda_from_delegated_account_and_owner_program,
    delegation_metadata_pda_from_delegated_account, delegation_record_pda_from_delegated_account,
    ephemeral_balance_pda_from_payer,
};

/// Delegate ephemeral balance
pub fn delegate_ephemeral_balance(
    payer: Pubkey,
    pubkey: Pubkey,
    args: DelegateEphemeralBalanceArgs,
) -> Instruction {
    let delegated_account = ephemeral_balance_pda_from_payer(&pubkey, args.index);
    let delegate_buffer_pda = delegate_buffer_pda_from_delegated_account_and_owner_program(
        &delegated_account,
        &crate::id(),
    );
    let delegation_record_pda = delegation_record_pda_from_delegated_account(&delegated_account);
    let delegation_metadata_pda =
        delegation_metadata_pda_from_delegated_account(&delegated_account);
    let mut data = DlpDiscriminator::DelegateEphemeralBalance.to_vec();
    data.extend_from_slice(&args.try_to_vec().unwrap());

    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(pubkey, true),
            AccountMeta::new(delegated_account, false),
            AccountMeta::new(delegate_buffer_pda, false),
            AccountMeta::new(delegation_record_pda, false),
            AccountMeta::new(delegation_metadata_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(crate::id(), false),
        ],
        data,
    }
}
