use solana_program::instruction::Instruction;
use solana_program::{instruction::AccountMeta, pubkey::Pubkey};

use crate::discriminator::DlpDiscriminator;
use crate::pda::fees_vault_pda;

/// Claim the accrued fees from the protocol fees vault.
pub fn protocol_claim_fees(admin: Pubkey) -> Instruction {
    let fees_vault_pda = fees_vault_pda();
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(admin, true),
            AccountMeta::new(fees_vault_pda, false),
        ],
        data: DlpDiscriminator::ProtocolClaimFees.to_vec(),
    }
}
