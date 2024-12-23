use borsh::BorshSerialize;
use solana_program::instruction::Instruction;
use solana_program::system_program;
use solana_program::{instruction::AccountMeta, pubkey::Pubkey};

use crate::args::DelegateArgs;
use crate::discriminator::DlpDiscriminator;
use crate::pda::{
    delegate_buffer_pda_from_delegated_account_and_owner_program,
    delegation_metadata_pda_from_delegated_account, delegation_record_pda_from_delegated_account,
};

/// Builds a delegate instruction
pub fn delegate(
    payer: Pubkey,
    delegated_account: Pubkey,
    owner: Option<Pubkey>,
    args: DelegateArgs,
) -> Instruction {
    let owner = owner.unwrap_or(system_program::id());
    let delegate_buffer_pda =
        delegate_buffer_pda_from_delegated_account_and_owner_program(&delegated_account, &owner);
    let delegation_record_pda = delegation_record_pda_from_delegated_account(&delegated_account);
    let delegation_metadata_pda =
        delegation_metadata_pda_from_delegated_account(&delegated_account);
    let mut data = DlpDiscriminator::Delegate.to_vec();
    data.extend_from_slice(&args.try_to_vec().unwrap());

    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(delegated_account, true),
            AccountMeta::new_readonly(owner, false),
            AccountMeta::new(delegate_buffer_pda, false),
            AccountMeta::new(delegation_record_pda, false),
            AccountMeta::new(delegation_metadata_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    }
}
