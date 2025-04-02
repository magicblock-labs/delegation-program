use solana_program::instruction::Instruction;
use solana_program::{bpf_loader_upgradeable, instruction::AccountMeta, pubkey::Pubkey};

use crate::discriminator::DlpDiscriminator;
use crate::pda::validator_fees_vault_pda_from_validator;

/// Close a validator fees vault PDA.
/// See [crate::processor::process_close_validator_fees_vault] for docs.
pub fn close_validator_fees_vault(
    payer: Pubkey,
    admin: Pubkey,
    validator_identity: Pubkey,
) -> Instruction {
    let validator_fees_vault_pda = validator_fees_vault_pda_from_validator(&validator_identity);
    let delegation_program_data =
        Pubkey::find_program_address(&[crate::ID.as_ref()], &bpf_loader_upgradeable::id()).0;
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(admin, true),
            AccountMeta::new_readonly(delegation_program_data, false),
            AccountMeta::new(validator_identity, false),
            AccountMeta::new(validator_fees_vault_pda, false),
        ],
        data: DlpDiscriminator::CloseValidatorFeesVault.to_vec(),
    }
}
