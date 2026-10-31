use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;

use crate::{
    compat::{Compatize, Modernize},
    legacy::pda::{
        delegation_metadata_pda_from_delegated_account,
        delegation_record_pda_from_delegated_account,
    },
    pda::{challenge_pda, pending_commitment_pda, state_buffer_pda},
    DlpV2Instruction,
};

/// Builds the instruction that finalizes one approved v2 commitment.
pub fn finalize_commitment(
    operator: Pubkey,
    delegation_cluster: Pubkey,
    account: Pubkey,
    commit_seq: u32,
) -> Instruction {
    finalize_commitment_with_state_authority(
        operator,
        delegation_cluster,
        account,
        commit_seq,
        None,
    )
}

/// Builds the instruction that finalizes a resolver-selected challenger state.
pub fn finalize_challenger_commitment(
    challenger: Pubkey,
    delegation_cluster: Pubkey,
    account: Pubkey,
    commit_seq: u32,
) -> Instruction {
    let challenge = challenge_pda(
        &account.compatize(),
        commit_seq,
        &challenger.compatize(),
    )
    .modernize();

    finalize_commitment_with_state_authority(
        challenger,
        delegation_cluster,
        account,
        commit_seq,
        Some(challenge),
    )
}

fn finalize_commitment_with_state_authority(
    state_authority: Pubkey,
    delegation_cluster: Pubkey,
    account: Pubkey,
    commit_seq: u32,
    challenge: Option<Pubkey>,
) -> Instruction {
    let mut accounts = vec![
        AccountMeta::new(state_authority, true),
        AccountMeta::new(
            pending_commitment_pda(&account.compatize(), commit_seq)
                .modernize(),
            false,
        ),
        AccountMeta::new(account, false),
        AccountMeta::new(
            delegation_record_pda_from_delegated_account(&account.compatize())
                .modernize(),
            false,
        ),
        AccountMeta::new(
            delegation_metadata_pda_from_delegated_account(
                &account.compatize(),
            )
            .modernize(),
            false,
        ),
        AccountMeta::new_readonly(
            state_buffer_pda(
                &account.compatize(),
                commit_seq,
                &state_authority.compatize(),
            )
            .modernize(),
            false,
        ),
        AccountMeta::new_readonly(delegation_cluster, false),
        AccountMeta::new_readonly(system_program::id(), false),
    ];
    if let Some(challenge) = challenge {
        accounts.push(AccountMeta::new_readonly(challenge, false));
    }

    Instruction {
        program_id: crate::id().modernize(),
        accounts,
        data: DlpV2Instruction::FinalizeCommitment.to_vec(),
    }
}
