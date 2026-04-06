use dlp::{
    consts::DELEGATION_PROGRAM_DATA_ID, discriminator::DlpDiscriminator,
    pda::validator_fees_vault_pda_from_validator,
};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

/// Close a validator fees vault PDA.
/// See [dlp::processor::process_close_validator_fees_vault] for docs.
pub fn close_validator_fees_vault(
    payer: Pubkey,
    admin: Pubkey,
    validator_identity: Pubkey,
) -> Instruction {
    let validator_fees_vault_pda =
        validator_fees_vault_pda_from_validator(&validator_identity);
    let delegation_program_data = DELEGATION_PROGRAM_DATA_ID.to_bytes().into();
    Instruction {
        program_id: dlp::id(),
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
