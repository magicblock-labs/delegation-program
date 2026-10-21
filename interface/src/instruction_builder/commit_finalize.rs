use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;
use wheels::{layout::Encodable, DataLayoutError};

use crate::{
    compat::{Compatize, Modernize},
    pda::delegation_state_pda,
    CommitFinalizeArgs, DlpV2Instruction,
};

/// Builds a direct-cluster commit that carries its payload inline.
pub fn commit_finalize(
    current_validator: Pubkey,
    delegation_cluster: Pubkey,
    delegated_account: Pubkey,
    args: CommitFinalizeArgs,
) -> Result<Instruction, DataLayoutError> {
    Ok(Instruction {
        program_id: crate::id().modernize(),
        accounts: vec![
            AccountMeta::new(current_validator, true),
            AccountMeta::new(delegated_account, false),
            AccountMeta::new(
                delegation_state_pda(&delegated_account.compatize())
                    .modernize(),
                false,
            ),
            AccountMeta::new_readonly(delegation_cluster, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: [DlpV2Instruction::CommitFinalize.to_vec(), args.encode()?]
            .concat(),
    })
}
