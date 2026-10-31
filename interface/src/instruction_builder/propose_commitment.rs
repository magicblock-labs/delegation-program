use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;
use wheels::layout::Encodable;

use crate::{
    compat::{Compatize, Modernize},
    legacy::pda::{
        delegation_metadata_pda_from_delegated_account,
        delegation_record_pda_from_delegated_account,
    },
    pda::{
        operator_bond_pda, pending_commitment_pda, state_buffer_pda,
        verifier_registry_pda,
    },
    DlpV2Instruction, ProposeCommitmentArgs,
};

/// Builds the instruction that proposes one v2 account-state commitment.
pub fn propose_commitment(
    current_validator: Pubkey,
    delegation_cluster: Pubkey,
    account: Pubkey,
    args: ProposeCommitmentArgs,
) -> Instruction {
    Instruction {
        program_id: crate::id().modernize(),
        accounts: vec![
            AccountMeta::new(current_validator, true),
            AccountMeta::new_readonly(
                operator_bond_pda(&delegation_cluster.compatize()).modernize(),
                false,
            ),
            AccountMeta::new(
                pending_commitment_pda(&account.compatize(), args.commit_seq)
                    .modernize(),
                false,
            ),
            AccountMeta::new_readonly(
                state_buffer_pda(
                    &account.compatize(),
                    args.commit_seq,
                    &current_validator.compatize(),
                )
                .modernize(),
                false,
            ),
            AccountMeta::new_readonly(account, false),
            AccountMeta::new_readonly(
                delegation_record_pda_from_delegated_account(
                    &account.compatize(),
                )
                .modernize(),
                false,
            ),
            AccountMeta::new_readonly(
                delegation_metadata_pda_from_delegated_account(
                    &account.compatize(),
                )
                .modernize(),
                false,
            ),
            AccountMeta::new_readonly(delegation_cluster, false),
            AccountMeta::new(
                verifier_registry_pda(&delegation_cluster.compatize())
                    .modernize(),
                false,
            ),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: [
            DlpV2Instruction::ProposeCommitment.to_vec(),
            args.encode().unwrap(),
        ]
        .concat(),
    }
}
