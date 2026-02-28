use dlp::{
    args::{DelegateArgs, DelegateWithActionsArgs},
    discriminator::DlpDiscriminator,
    pda::{
        delegate_buffer_pda_from_delegated_account_and_owner_program,
        delegation_metadata_pda_from_delegated_account,
        delegation_record_pda_from_delegated_account,
    },
};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    system_program,
};

use super::types::{EncryptableAccountMeta, PostDelegationInstruction};

/// See [dlp::processor::process_delegate_with_actions] for docs.
pub fn delegate_with_actions(
    payer: Pubkey,
    delegated_account: Pubkey,
    owner: Option<Pubkey>,
    delegate: DelegateArgs,
    actions: Vec<PostDelegationInstruction>,
) -> Instruction {
    let (actions, signers) =
        create_post_delegation_actions(actions, delegate.validator);

    Instruction {
        program_id: dlp::id(),
        accounts: {
            let owner = owner.unwrap_or(system_program::id());
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
                    AccountMeta::new(delegated_account, true),
                    AccountMeta::new_readonly(owner, false),
                    AccountMeta::new(delegate_buffer_pda, false),
                    AccountMeta::new(delegation_record_pda, false),
                    AccountMeta::new(delegation_metadata_pda, false),
                    AccountMeta::new_readonly(system_program::id(), false),
                ],
                signers,
            ]
            .concat()
        },
        data: {
            let args = DelegateWithActionsArgs { delegate, actions };
            let mut data = DlpDiscriminator::DelegateWithActions.to_vec();
            data.extend_from_slice(&bincode::serialize(&args).unwrap());
            data
        },
    }
}

pub fn create_post_delegation_actions(
    instructions: Vec<PostDelegationInstruction>,
    validator: Option<Pubkey>,
) -> (dlp::args::PostDelegationActions, Vec<AccountMeta>) {
    use dlp::args::MaybeEncryptedInstruction;

    let mut signers: Vec<AccountMeta> = Vec::new();
    let mut add_to_signers = |meta: &EncryptableAccountMeta| {
        assert!(meta.account_meta.is_signer, "AccountMeta must be a signer");
        assert!(!meta.is_encryptable, "signer must not be encryptable");
        let Some(found) = signers
            .iter_mut()
            .find(|m| m.pubkey == meta.account_meta.pubkey)
        else {
            signers.push(meta.account_meta.clone());
            return;
        };

        found.is_signer |= meta.account_meta.is_signer;
        found.is_writable |= meta.account_meta.is_writable;
    };

    let mut non_signers: Vec<EncryptableAccountMeta> = Vec::new();
    let mut add_to_non_signers = |meta: &EncryptableAccountMeta| {
        assert!(
            !meta.account_meta.is_signer,
            "AccountMeta must not be a signer"
        );
        let Some(found) = non_signers
            .iter_mut()
            .find(|m| m.account_meta.pubkey == meta.account_meta.pubkey)
        else {
            non_signers.push(meta.clone());
            return;
        };

        found.is_encryptable |= meta.is_encryptable;
        found.account_meta.is_writable |= meta.account_meta.is_writable;
    };

    for meta in instructions
        .iter()
        .flat_map(|ix| ix.accounts.iter())
        .filter(|meta| meta.account_meta.is_signer)
    {
        add_to_signers(meta);
    }

    for ix in instructions.iter() {
        add_to_non_signers(&EncryptableAccountMeta {
            account_meta: AccountMeta::new_readonly(
                ix.program_id.pubkey,
                false,
            ),
            is_encryptable: ix.program_id.is_encryptable,
        });
        for meta in ix
            .accounts
            .iter()
            .filter(|meta| !meta.account_meta.is_signer)
        {
            let Some(found) = signers
                .iter_mut()
                .find(|m| m.pubkey == meta.account_meta.pubkey)
            else {
                add_to_non_signers(meta);
                continue;
            };

            found.is_writable |= meta.account_meta.is_writable;
        }
    }

    if signers.len() + non_signers.len() > dlp::compact::MAX_PUBKEYS as usize {
        panic!(
            "delegate_with_actions supports at most {} unique pubkeys",
            dlp::compact::MAX_PUBKEYS
        );
    }

    let index_of = |pk: &Pubkey| -> u8 {
        if let Some(index) = signers.iter().position(|s| &s.pubkey == pk) {
            return index as u8;
        }
        signers.len() as u8
            + non_signers
                .iter()
                .position(|ns| &ns.account_meta.pubkey == pk)
                .unwrap() as u8
    };

    let compact_instructions: Vec<MaybeEncryptedInstruction> = instructions
        .into_iter()
        .map(|ix| MaybeEncryptedInstruction {
            program_id: index_of(&ix.program_id.pubkey),

            accounts: ix
                .accounts
                .into_iter()
                .map(|meta| {
                    dlp::compact::AccountMeta::try_new(
                        index_of(&meta.account_meta.pubkey),
                        meta.account_meta.is_signer,
                        meta.account_meta.is_writable,
                    )
                    .expect("compact account index must fit in 6 bits")
                })
                .collect(),

            data: ix.data.encrypt(&validator),
        })
        .collect();

    (
        dlp::args::PostDelegationActions {
            signers: signers.iter().map(|s| s.pubkey).collect(),

            non_signers: non_signers
                .into_iter()
                .map(|ns| ns.encrypt(&validator))
                .collect(),

            instructions: compact_instructions,
        },
        signers,
    )
}

#[cfg(test)]
mod tests {
    use dlp::args::MaybeEncryptedPubkey;
    use solana_sdk::{signature::Keypair, signer::Signer};

    use super::*;
    use crate::instruction_builder::types::{Encryptable, EncryptableFrom};

    #[test]
    fn test_compact_post_delegation_actions() {
        let a = Pubkey::new_from_array([1; 32]); // 0: signer
        let b = Pubkey::new_from_array([2; 32]); // 1: non-signer
        let c = Pubkey::new_from_array([3; 32]); // 2: signer
        let d = Pubkey::new_from_array([4; 32]); // 3: non-signer
        let e = Pubkey::new_from_array([5; 32]); // 4: signer

        let instructions = vec![PostDelegationInstruction {
            program_id: d.encrypted(),
            accounts: vec![
                AccountMeta::new_readonly(a, true).cleartext(), // a
                AccountMeta::new(c, true).cleartext(),          // c
                AccountMeta::new_readonly(b, false).encrypted(), // b
                AccountMeta::new_readonly(e, true).cleartext(), // e
                AccountMeta::new(d, false).encrypted(),         // d
            ],
            data: vec![9].encrypted_from(1),
        }];

        let validator = Keypair::new();
        let (actions, _meta_signers) = create_post_delegation_actions(
            instructions,
            Some(validator.pubkey()),
        );

        // indices: a, c, e, d, b
        //          0, 1, 2, 3, 4

        assert_eq!(actions.signers.len(), 3);
        assert_eq!(actions.signers[0], a); // signer
        assert_eq!(actions.signers[1], c); // signer
        assert_eq!(actions.signers[2], e); // signer

        if false {
            let non_signer_pubkeys: Vec<_> = actions
                .non_signers
                .iter()
                .map(|key| match key {
                    MaybeEncryptedPubkey::ClearText(pubkey) => *pubkey,
                    MaybeEncryptedPubkey::Encrypted(_) => {
                        panic!("there must not be any encrypted pubkeys")
                    }
                })
                .collect();

            assert_eq!(non_signer_pubkeys[0], d); // non-signer
            assert_eq!(non_signer_pubkeys[1], b); // non-signer
        } else {
            assert_eq!(actions.non_signers.len(), 2); // non-signer
        }

        // old->new mapping: a(0)->0, b(1)->4, c(2)->1, d(3)->3, e(4)->2
        assert_eq!(actions.instructions[0].program_id, 3); // d
        assert_eq!(actions.instructions[0].accounts[0].key(), 0); // a
        assert_eq!(actions.instructions[0].accounts[1].key(), 1); // c
        assert_eq!(actions.instructions[0].accounts[2].key(), 4); // b
        assert_eq!(actions.instructions[0].accounts[3].key(), 2); // e
        assert_eq!(actions.instructions[0].accounts[4].key(), 3); // d
    }

    #[test]
    fn test_instruction_encrypted() {
        let signer = Pubkey::new_unique();
        let nonsigner = Pubkey::new_unique();
        let program_id = Pubkey::new_unique();

        let enc = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new_readonly(signer, true),
                AccountMeta::new(nonsigner, false),
            ],
            data: vec![1, 2, 3],
        }
        .encrypted();

        assert_eq!(enc.program_id.pubkey, program_id);
        assert!(enc.program_id.is_encryptable);
        assert!(enc.accounts[0].is_encryptable);
        assert!(enc.accounts[1].is_encryptable);
        assert_eq!(enc.data.encrypt_begin_offset, 0);
        assert_eq!(enc.data.data, vec![1, 2, 3]);
    }

    #[test]
    fn test_instruction_encrypted_from() {
        let signer = Pubkey::new_unique();
        let nonsigner = Pubkey::new_unique();
        let program_id = Pubkey::new_unique();

        let enc = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new_readonly(signer, true),
                AccountMeta::new(nonsigner, false),
            ],
            data: vec![9, 9, 9, 9, 9, 9],
        }
        .encrypted_from(4);

        assert_eq!(enc.program_id.pubkey, program_id);
        assert!(enc.program_id.is_encryptable);
        assert!(enc.accounts[0].is_encryptable);
        assert!(enc.accounts[1].is_encryptable);
        assert_eq!(enc.data.encrypt_begin_offset, 4);
        assert_eq!(enc.data.data, vec![9, 9, 9, 9, 9, 9]);
    }
}
