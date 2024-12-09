use solana_program::instruction::Instruction;
use solana_program::system_program;
use solana_program::{instruction::AccountMeta, pubkey::Pubkey};

use crate::discriminant::DlpDiscriminant;
use crate::pda::fees_vault_pda;

/// Initialize the fees vault PDA.
pub fn init_fees_vault(payer: Pubkey) -> Instruction {
    let fees_vault = fees_vault_pda();
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(fees_vault, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: DlpDiscriminant::InitFeesVault.to_vec(),
    }
}
