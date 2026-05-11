use dlp::{
    args::ValidatorClaimFeesArgs,
    discriminator::DlpDiscriminator,
    pda::{fees_vault_pda, validator_fees_vault_pda_from_validator},
};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

use crate::compat::{borsh::to_vec, Compatize, Modernize};

/// Claim the accrued fees from the fees vault.
/// See [dlp::processor::process_validator_claim_fees] for docs.
pub fn validator_claim_fees(
    validator: Pubkey,
    amount: Option<u64>,
) -> Instruction {
    let args = ValidatorClaimFeesArgs { amount };
    let validator_compat = validator.compatize();
    let fees_vault_pda = fees_vault_pda().modernize();
    let validator_fees_vault_pda =
        validator_fees_vault_pda_from_validator(&validator_compat).modernize();
    Instruction {
        program_id: dlp::id().modernize(),
        accounts: vec![
            AccountMeta::new(validator, true),
            AccountMeta::new(fees_vault_pda, false),
            AccountMeta::new(validator_fees_vault_pda, false),
        ],
        data: [
            DlpDiscriminator::ValidatorClaimFees.to_vec(),
            to_vec(&args).unwrap(),
        ]
        .concat(),
    }
}
