use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;
use wheels::layout::Encodable;

use crate::{
    compat::{Compatize, Modernize},
    pda::delegation_record_pda_from_delegated_account,
    v2::{
        pda::{
            operator_bond_pda, pending_commitment_pda, protocol_config_pda,
            state_buffer_pda, verifier_registry_pda,
        },
        DlpV2Instruction, PostCommitmentArgs,
    },
};

/// Builds the instruction that posts one v2 account-state commitment.
pub fn post_commitment(
    operator: Pubkey,
    account: Pubkey,
    args: PostCommitmentArgs,
) -> Instruction {
    Instruction {
        program_id: crate::id().modernize(),
        accounts: vec![
            AccountMeta::new(operator, true),
            AccountMeta::new_readonly(
                operator_bond_pda(&operator.compatize()).modernize(),
                false,
            ),
            AccountMeta::new(
                pending_commitment_pda(&account.compatize(), args.commit_id)
                    .modernize(),
                false,
            ),
            AccountMeta::new_readonly(
                state_buffer_pda(
                    &account.compatize(),
                    args.commit_id,
                    &operator.compatize(),
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
            AccountMeta::new_readonly(protocol_config_pda().modernize(), false),
            AccountMeta::new(verifier_registry_pda().modernize(), false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: [
            DlpV2Instruction::PostCommitment.to_vec(),
            args.encode().unwrap(),
        ]
        .concat(),
    }
}
