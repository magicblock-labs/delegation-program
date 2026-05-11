use borsh::to_vec;
use dlp::{
    args::{DelegateArgs, DelegateWithActionsArgs, PostDelegationActions},
    discriminator::DlpDiscriminator,
    pda::{
        delegate_buffer_pda_from_delegated_account_and_owner_program,
        delegation_metadata_pda_from_delegated_account,
        delegation_record_pda_from_delegated_account,
    },
};
use solana_program::instruction::{AccountMeta, Instruction};
use solana_sdk_ids::system_program;

use crate::compat::{borsh, Compatize, Modernize, Pubkey};

pub fn delegate_with_actions(
    payer: Pubkey,
    delegated_account: Pubkey,
    owner: Option<Pubkey>,
    delegate: DelegateArgs,
    actions: PostDelegationActions,
) -> Instruction {
    // Note that it is OK to assume that all signers
    // are readonly because DelegateWithActions needs to validate
    // the signer-status, not mutability status.
    let actions_signers: Vec<AccountMeta> = actions
        .signers
        .iter()
        .copied()
        .map(|pk| AccountMeta::new_readonly(pk.into(), true))
        .collect();

    Instruction {
        program_id: dlp::id().modernize(),

        accounts: {
            let owner =
                owner.unwrap_or_else(|| system_program::id().compatize());
            let payer = payer.modernize();
            let delegated_account_modern = delegated_account.modernize();
            let owner_modern = owner.modernize();
            let delegate_buffer_pda =
                delegate_buffer_pda_from_delegated_account_and_owner_program(
                    &delegated_account,
                    &owner,
                );
            let delegation_record_pda =
                delegation_record_pda_from_delegated_account(
                    &delegated_account,
                );
            let delegation_metadata_pda =
                delegation_metadata_pda_from_delegated_account(
                    &delegated_account,
                );

            [
                vec![
                    AccountMeta::new(payer, true),
                    AccountMeta::new(delegated_account_modern, true),
                    AccountMeta::new_readonly(owner_modern, false),
                    AccountMeta::new(delegate_buffer_pda.modernize(), false),
                    AccountMeta::new(delegation_record_pda.modernize(), false),
                    AccountMeta::new(
                        delegation_metadata_pda.modernize(),
                        false,
                    ),
                    AccountMeta::new_readonly(system_program::id(), false),
                ],
                actions_signers,
            ]
            .concat()
        },

        data: {
            let args = DelegateWithActionsArgs { delegate, actions };
            let mut data = DlpDiscriminator::DelegateWithActions.to_vec();
            data.extend_from_slice(&to_vec(&args).unwrap());
            data
        },
    }
}
