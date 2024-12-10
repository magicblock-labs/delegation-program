use solana_program::instruction::Instruction;
use solana_program::system_program;
use solana_program::{instruction::AccountMeta, pubkey::Pubkey};

use crate::discriminator::DlpDiscriminator;
use crate::pda::validator_fees_vault_pda_from_validator;

/// Initialize a validator fees vault PDA.
pub fn init_validator_fees_vault(
    payer: Pubkey,
    admin: Pubkey,
    validator_identity: Pubkey,
) -> Instruction {
    let validator_fees_vault_pda = validator_fees_vault_pda_from_validator(&validator_identity);
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(admin, true),
            AccountMeta::new(validator_identity, false),
            AccountMeta::new(validator_fees_vault_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: DlpDiscriminator::InitValidatorFeesVault.to_vec(),
    }
}
