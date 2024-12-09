use borsh::BorshSerialize;
use solana_program::instruction::Instruction;
use solana_program::{instruction::AccountMeta, pubkey::Pubkey};

use crate::args::ValidatorClaimFeesArgs;
use crate::discriminant::DlpDiscriminant;
use crate::pda::{fees_vault_pda, validator_fees_vault_pda_from_validator};

/// Claim the accrued fees from the fees vault.
pub fn validator_claim_fees(validator: Pubkey, amount: Option<u64>) -> Instruction {
    let args = ValidatorClaimFeesArgs { amount };
    let fees_vault_pda = fees_vault_pda();
    let validator_fees_vault_pda = validator_fees_vault_pda_from_validator(&validator);
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(validator, true),
            AccountMeta::new(fees_vault_pda, false),
            AccountMeta::new(validator_fees_vault_pda, false),
        ],
        data: [
            DlpDiscriminant::ValidatorClaimFees.to_vec(),
            args.try_to_vec().unwrap(),
        ]
        .concat(),
    }
}
