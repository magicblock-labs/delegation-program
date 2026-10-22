use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;
use wheels::{layout::Encodable, DataLayoutError};

use crate::{
    compat::{Compatize, Modernize},
    pda::{delegation_state_pda, undelegate_buffer_pda},
    DlpV2Instruction, UndelegateArgs,
};

/// Builds the shared undelegation instruction.
pub fn undelegate(
    current_validator: Pubkey,
    delegated_account: Pubkey,
    original_owner: Pubkey,
    delegation_cluster: Pubkey,
    delegation_rent_payer: Pubkey,
) -> Result<Instruction, DataLayoutError> {
    let (undelegate_buffer, undelegate_buffer_bump) =
        undelegate_buffer_pda(&delegated_account.compatize());

    Ok(Instruction {
        program_id: crate::id().modernize(),
        accounts: vec![
            AccountMeta::new(current_validator, true),
            AccountMeta::new(delegated_account, false),
            AccountMeta::new_readonly(original_owner, false),
            AccountMeta::new(undelegate_buffer.modernize(), false),
            AccountMeta::new(
                delegation_state_pda(&delegated_account.compatize())
                    .modernize(),
                false,
            ),
            AccountMeta::new_readonly(delegation_cluster, false),
            AccountMeta::new(delegation_rent_payer, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: [
            DlpV2Instruction::Undelegate.to_vec(),
            UndelegateArgs {
                undelegate_buffer_bump,
            }
            .encode()?,
        ]
        .concat(),
    })
}
