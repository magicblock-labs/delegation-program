use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;
use wheels::layout::Encodable;

use crate::{
    compat::Modernize,
    v2::{
        pda::{delegation_cluster_pda, verifier_registry_pda},
        DlpV2Instruction, InitDelegationClusterArgs,
    },
};

/// Builds the instruction that creates one delegation cluster.
///
/// The cluster identity must sign to prove control of its PDA namespace.
pub fn init_delegation_cluster(
    config_authority: Pubkey,
    cluster_identity: Pubkey,
    args: InitDelegationClusterArgs,
) -> Instruction {
    let delegation_cluster = delegation_cluster_pda(&cluster_identity);
    let mut accounts = vec![
        AccountMeta::new(config_authority, true),
        AccountMeta::new_readonly(cluster_identity.modernize(), true),
        AccountMeta::new(delegation_cluster.modernize(), false),
        AccountMeta::new_readonly(system_program::id(), false),
    ];

    if args.challenge_config.is_some() {
        accounts.push(AccountMeta::new(
            verifier_registry_pda(&delegation_cluster).modernize(),
            false,
        ));
    }

    Instruction {
        program_id: crate::id().modernize(),
        accounts,
        data: [
            DlpV2Instruction::InitDelegationCluster.to_vec(),
            args.encode().unwrap(),
        ]
        .concat(),
    }
}
