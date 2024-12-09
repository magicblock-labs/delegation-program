use borsh::BorshSerialize;
use solana_program::instruction::Instruction;
use solana_program::system_program;
use solana_program::{instruction::AccountMeta, pubkey::Pubkey};

use crate::args::DelegateEphemeralBalanceArgs;
use crate::consts::BUFFER;
use crate::discriminant::DlpDiscriminant;
use crate::pda::{
    delegation_metadata_pda_from_delegated_account, delegation_record_pda_from_delegated_account,
    ephemeral_balance_pda_from_payer,
};

/// Delegate ephemeral balance
pub fn delegate_ephemeral_balance(
    payer: Pubkey,
    pubkey: Pubkey,
    args: DelegateEphemeralBalanceArgs,
) -> Instruction {
    let delegate_account = ephemeral_balance_pda_from_payer(&pubkey, args.index);
    let buffer =
        Pubkey::find_program_address(&[BUFFER, &delegate_account.to_bytes()], &crate::id());
    let delegation_record_pda = delegation_record_pda_from_delegated_account(&delegate_account);
    let delegation_metadata_pda = delegation_metadata_pda_from_delegated_account(&delegate_account);
    let mut data = DlpDiscriminant::DelegateEphemeralBalance.to_vec();
    data.extend_from_slice(&args.try_to_vec().unwrap());

    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(pubkey, true),
            AccountMeta::new(delegate_account, false),
            AccountMeta::new(buffer.0, false),
            AccountMeta::new(delegation_record_pda, false),
            AccountMeta::new(delegation_metadata_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(crate::id(), false),
        ],
        data,
    }
}
