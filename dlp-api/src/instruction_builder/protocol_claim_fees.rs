use dlp::{
    consts::DELEGATION_PROGRAM_DATA_ID, discriminator::DlpDiscriminator,
    pda::fees_vault_pda,
};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

/// Claim the accrued fees from the protocol fees vault.
/// See [dlp::processor::process_protocol_claim_fees] for docs.
pub fn protocol_claim_fees(admin: Pubkey) -> Instruction {
    let fees_vault_pda = fees_vault_pda();
    let delegation_program_data = DELEGATION_PROGRAM_DATA_ID.to_bytes().into();
    Instruction {
        program_id: dlp::id(),
        accounts: vec![
            AccountMeta::new(admin, true),
            AccountMeta::new(fees_vault_pda, false),
            AccountMeta::new_readonly(delegation_program_data, false),
        ],
        data: DlpDiscriminator::ProtocolClaimFees.to_vec(),
    }
}
