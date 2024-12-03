use borsh::BorshSerialize;
use solana_program::instruction::Instruction;
use solana_program::{instruction::AccountMeta, pubkey::Pubkey};

use crate::consts::FEES_VAULT;
use crate::discriminant::DlpDiscriminant;
use crate::pda::validator_fees_vault_pda_from_pubkey;
use crate::processor::ValidatorClaimFeesArgs;

/// Claim the accrued fees from the fees vault.
pub fn validator_claim_fees(validator: Pubkey, amount: Option<u64>) -> Instruction {
    let args = ValidatorClaimFeesArgs { amount };
    let fees_vault = Pubkey::find_program_address(&[FEES_VAULT], &crate::id()).0;
    let validator_fees_vault = validator_fees_vault_pda_from_pubkey(&validator);
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(validator, true),
            AccountMeta::new(fees_vault, false),
            AccountMeta::new(validator_fees_vault, false),
        ],
        data: [
            DlpDiscriminant::ValidatorClaimFees.to_vec(),
            args.try_to_vec().unwrap(),
        ]
        .concat(),
    }
}
