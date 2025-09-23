use borsh::to_vec;
use solana_program::instruction::Instruction;
use solana_program::{instruction::AccountMeta, pubkey::Pubkey};

use crate::args::DelegateCompressedArgs;
use crate::discriminator::DlpDiscriminator;

/// Builds a delegate instruction
/// See [crate::processor::process_delegate_compressed] for docs.
pub fn delegate_compressed(
    payer: Pubkey,
    delegated_account: Pubkey,
    owner: Pubkey,
    args: DelegateCompressedArgs,
) -> Instruction {
    let mut data = DlpDiscriminator::DelegateCompressed.to_vec();
    data.extend_from_slice(&to_vec(&args).unwrap());

    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(delegated_account, true),
            AccountMeta::new_readonly(owner, false),
        ],
        data,
    }
}
