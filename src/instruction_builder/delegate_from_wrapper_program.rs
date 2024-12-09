use solana_program::instruction::Instruction;
use solana_program::{instruction::AccountMeta, pubkey::Pubkey};

use crate::consts::BUFFER;
use crate::pda::{
    delegation_metadata_pda_from_delegated_account, delegation_record_pda_from_delegated_account,
};

/// Builds a delegate instruction.
pub fn delegate_from_wrapper_program(
    payer: Pubkey,
    delegate_account: Pubkey,
    system_program: Pubkey,
    delegation_program: Pubkey,
    owner_program: Pubkey,
    discriminator: Vec<u8>,
) -> Instruction {
    let buffer =
        Pubkey::find_program_address(&[BUFFER, &delegate_account.to_bytes()], &owner_program);
    let delegation_record = delegation_record_pda_from_delegated_account(&delegate_account);
    let delegation_metadata = delegation_metadata_pda_from_delegated_account(&delegate_account);

    Instruction {
        program_id: owner_program,
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(buffer.0, false),
            AccountMeta::new(delegation_record, false),
            AccountMeta::new(delegation_metadata, false),
            AccountMeta::new(delegate_account, false),
            AccountMeta::new_readonly(owner_program, false),
            AccountMeta::new_readonly(delegation_program, false),
            AccountMeta::new_readonly(system_program, false),
        ],
        data: discriminator,
    }
}
