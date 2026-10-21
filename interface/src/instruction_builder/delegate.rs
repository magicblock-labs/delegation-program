use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;
use wheels::layout::Encodable;

use crate::{
    compat::{Compatize, Modernize},
    legacy::pda::DELEGATE_BUFFER_TAG,
    pda::{delegation_state_pda, DELEGATION_STATE_SEED},
    DelegateArgs, DlpV2Instruction,
};

/// Builds the shared instruction that delegates an account to one cluster.
pub fn delegate(
    payer: Pubkey,
    delegated_account: Pubkey,
    original_owner: Pubkey,
    delegation_cluster: Pubkey,
    mut args: DelegateArgs,
) -> Instruction {
    let (delegate_buffer, delegate_buffer_bump) = Pubkey::find_program_address(
        &[DELEGATE_BUFFER_TAG, delegated_account.as_ref()],
        &original_owner,
    );
    let (_, delegation_state_bump) = Pubkey::find_program_address(
        &[DELEGATION_STATE_SEED, delegated_account.as_ref()],
        &crate::id().modernize(),
    );
    args.delegate_buffer_bump = delegate_buffer_bump;
    args.delegation_state_bump = delegation_state_bump;

    Instruction {
        program_id: crate::id().modernize(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(delegated_account, true),
            AccountMeta::new_readonly(original_owner, false),
            AccountMeta::new(delegate_buffer, false),
            AccountMeta::new(
                delegation_state_pda(&delegated_account.compatize())
                    .modernize(),
                false,
            ),
            AccountMeta::new_readonly(delegation_cluster, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: [DlpV2Instruction::Delegate.to_vec(), args.encode().unwrap()]
            .concat(),
    }
}
