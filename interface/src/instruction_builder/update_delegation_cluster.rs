use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use wheels::layout::Encodable;

use crate::{compat::Modernize, DlpV2Instruction, UpdateDelegationClusterArgs};

/// Builds the instruction that updates challenge policy for future work.
pub fn update_delegation_cluster(
    config_authority: Pubkey,
    delegation_cluster: Pubkey,
    args: UpdateDelegationClusterArgs,
) -> Instruction {
    Instruction {
        program_id: crate::id().modernize(),
        accounts: vec![
            AccountMeta::new_readonly(config_authority, true),
            AccountMeta::new(delegation_cluster, false),
        ],
        data: [
            DlpV2Instruction::UpdateDelegationCluster.to_vec(),
            args.encode().unwrap(),
        ]
        .concat(),
    }
}
