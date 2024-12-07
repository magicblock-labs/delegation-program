use solana_program::instruction::Instruction;
use solana_program::system_program;
use solana_program::{instruction::AccountMeta, pubkey::Pubkey};

use crate::consts::FEES_VAULT;
use crate::discriminator::DlpDiscriminator;

/// Initialize the fees vault PDA.
pub fn init_fees_vault(payer: Pubkey) -> Instruction {
    let fees_vault = Pubkey::find_program_address(&[FEES_VAULT], &crate::id()).0;
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(fees_vault, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: DlpDiscriminator::InitFeesVault.to_vec(),
    }
}
