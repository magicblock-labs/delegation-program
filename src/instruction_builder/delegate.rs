use borsh::BorshSerialize;
use solana_program::instruction::Instruction;
use solana_program::system_program;
use solana_program::{instruction::AccountMeta, pubkey::Pubkey};

use crate::args::DelegateArgs;
use crate::consts::BUFFER;
use crate::discriminant::DlpDiscriminant;
use crate::pda::{
    delegation_metadata_pda_from_delegated_account, delegation_record_pda_from_delegated_account,
};

/// Builds a delegate instruction
pub fn delegate(
    payer: Pubkey,
    delegate_account: Pubkey,
    owner: Option<Pubkey>,
    args: DelegateArgs,
) -> Instruction {
    let owner = owner.unwrap_or(system_program::id());
    let buffer = Pubkey::find_program_address(&[BUFFER, &delegate_account.to_bytes()], &owner);
    let delegation_record_pda = delegation_record_pda_from_delegated_account(&delegate_account);
    let delegation_metadata_pda = delegation_metadata_pda_from_delegated_account(&delegate_account);
    let mut data = DlpDiscriminant::Delegate.to_vec();
    data.extend_from_slice(&args.try_to_vec().unwrap());

    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(delegate_account, true),
            AccountMeta::new_readonly(owner, false),
            AccountMeta::new(buffer.0, false),
            AccountMeta::new(delegation_record_pda, false),
            AccountMeta::new(delegation_metadata_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    }
}
