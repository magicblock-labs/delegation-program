use solana_program::instruction::Instruction;
use solana_program::{instruction::AccountMeta, pubkey::Pubkey};

use crate::consts::BUFFER;
use crate::pda::{
    delegation_metadata_pda_from_delegated_account, delegation_record_pda_from_delegated_account,
};

/// Builds an allow_undelegate account instruction.
pub fn allow_undelegate(
    delegated_account: Pubkey,
    owner_program: Pubkey,
    discriminator: Vec<u8>,
) -> Instruction {
    let delegation_record_pda = delegation_record_pda_from_delegated_account(&delegated_account);
    let delegation_metadata_pda =
        delegation_metadata_pda_from_delegated_account(&delegated_account);
    let buffer =
        Pubkey::find_program_address(&[BUFFER, &delegated_account.to_bytes()], &owner_program).0;
    Instruction {
        program_id: owner_program,
        accounts: vec![
            AccountMeta::new_readonly(delegated_account, false),
            AccountMeta::new_readonly(delegation_record_pda, false),
            AccountMeta::new(delegation_metadata_pda, false),
            AccountMeta::new_readonly(buffer, false),
            AccountMeta::new_readonly(crate::id(), false),
        ],
        data: discriminator,
    }
}
