use dlp::{
    discriminator::DlpDiscriminator,
    pda::{
        magic_fee_vault_pda_from_validator,
        validator_fees_vault_pda_from_validator,
    },
};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    system_program,
};

/// Initialize a magic fee vault PDA for a validator.
/// See [crate::processor::process_init_magic_fee_vault] for docs.
pub fn init_magic_fee_vault(
    payer: Pubkey,
    validator_identity: Pubkey,
) -> Instruction {
    let validator_fees_vault =
        validator_fees_vault_pda_from_validator(&validator_identity);
    let magic_fee_vault =
        magic_fee_vault_pda_from_validator(&validator_identity);
    Instruction {
        program_id: dlp::id(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(validator_identity, true),
            AccountMeta::new_readonly(validator_fees_vault, false),
            AccountMeta::new(magic_fee_vault, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: DlpDiscriminator::InitMagicFeeVault.to_vec(),
    }
}
