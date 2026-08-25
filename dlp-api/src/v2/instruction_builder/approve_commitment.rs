use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

use crate::{
    compat::{Compatize, Modernize},
    v2::{
        pda::{pending_commitment_pda, verifier_bond_pda},
        DlpV2Instruction,
    },
};

/// Builds the instruction that approves one v2 account-state commitment.
pub fn approve_commitment(
    verifier: Pubkey,
    account: Pubkey,
    commit_id: u64,
) -> Instruction {
    Instruction {
        program_id: crate::id().modernize(),
        accounts: vec![
            AccountMeta::new_readonly(verifier, true),
            AccountMeta::new_readonly(
                verifier_bond_pda(&verifier.compatize()).modernize(),
                false,
            ),
            AccountMeta::new(
                pending_commitment_pda(&account.compatize(), commit_id)
                    .modernize(),
                false,
            ),
        ],
        data: DlpV2Instruction::ApproveCommitment.to_vec(),
    }
}
