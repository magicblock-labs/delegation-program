use dlp::{discriminator::DlpDiscriminator, pda::fees_vault_pda};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    system_program,
};

/// Initialize the fees vault PDA.
/// See [dlp::processor::process_init_protocol_fees_vault] for docs.
pub fn init_protocol_fees_vault(payer: Pubkey) -> Instruction {
    let fees_vault_pda = fees_vault_pda();
    Instruction {
        program_id: dlp::id(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(fees_vault_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: DlpDiscriminator::InitProtocolFeesVault.to_vec(),
    }
}
