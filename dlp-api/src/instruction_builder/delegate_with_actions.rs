#![cfg(feature = "encryption")]

use borsh::to_vec;
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

use super::types::{Encrypt, PostDelegationInstruction};

/// See [dlp::processor::process_delegate_with_actions] for docs.
pub fn delegate_with_actions(
    payer: Pubkey,
    delegated_account: Pubkey,
    owner: Option<Pubkey>,
    delegate: DelegateArgs,
    actions: Vec<PostDelegationInstruction>,
) -> Instruction {
    let encrypt_key = delegate
        .validator
        .expect("validator must be provided for encryption");
    let (actions, signers) = actions
        .encrypt(&encrypt_key)
        .expect("post-delegation actions encryption failed");

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
            data.extend_from_slice(&to_vec(&args).unwrap());
            data
        },
    }
}

#[cfg(test)]
mod tests {
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
        let (actions, _meta_signers) = instructions
            .encrypt(&validator.pubkey())
            .expect("post-delegation actions encryption failed");

        // indices: a, c, e, d, b
        //          0, 1, 2, 3, 4

        assert_eq!(actions.signers.len(), 3);
        assert_eq!(actions.signers[0], a.to_bytes()); // signer
        assert_eq!(actions.signers[1], c.to_bytes()); // signer
        assert_eq!(actions.signers[2], e.to_bytes()); // signer

        assert_eq!(actions.non_signers.len(), 2); // non-signer

        // old->new mapping: a(0)->0, b(1)->4, c(2)->1, d(3)->3, e(4)->2
        assert_eq!(actions.instructions[0].program_id, 3); // d
        let accounts = &actions.instructions[0].accounts;
        assert!(matches!(
            accounts[2],
            dlp::args::MaybeEncryptedAccountMeta::Encrypted(_)
        ));
        assert!(matches!(
            accounts[4],
            dlp::args::MaybeEncryptedAccountMeta::Encrypted(_)
        ));

        let dlp::args::MaybeEncryptedAccountMeta::ClearText(a_meta) =
            accounts[0]
        else {
            panic!("expected cleartext account meta for a");
        };
        let dlp::args::MaybeEncryptedAccountMeta::ClearText(c_meta) =
            accounts[1]
        else {
            panic!("expected cleartext account meta for c");
        };
        let dlp::args::MaybeEncryptedAccountMeta::ClearText(e_meta) =
            accounts[3]
        else {
            panic!("expected cleartext account meta for e");
        };

        assert_eq!(a_meta.key(), 0); // a
        assert_eq!(c_meta.key(), 1); // c
        assert_eq!(e_meta.key(), 2); // e
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
