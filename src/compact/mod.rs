mod account_meta;
mod instruction;

pub use account_meta::*;
pub use instruction::*;

use crate::args::{
    EncryptedBuffer, MaybeEncryptedAccountMeta, MaybeEncryptedInstruction,
    MaybeEncryptedIxData, MaybeEncryptedPubkey, PostDelegationActions,
};

pub trait ClearText: Sized {
    type Output;

    fn cleartext(self) -> Self::Output;
}

impl ClearText for Vec<u8> {
    type Output = MaybeEncryptedIxData;

    fn cleartext(self) -> Self::Output {
        MaybeEncryptedIxData {
            prefix: self,
            suffix: EncryptedBuffer::default(),
        }
    }
}

impl ClearText for Vec<solana_instruction::Instruction> {
    type Output = PostDelegationActions;

    fn cleartext(self) -> Self::Output {
        let mut signers: Vec<solana_instruction::AccountMeta> = Vec::new();
        let mut non_signers: Vec<solana_instruction::AccountMeta> = Vec::new();

        let mut add_to_signers = |meta: &solana_instruction::AccountMeta| {
            assert!(meta.is_signer, "AccountMeta must be a signer");
            let Some(found) =
                signers.iter_mut().find(|m| m.pubkey == meta.pubkey)
            else {
                signers.push(meta.clone());
                return;
            };

            found.is_signer |= meta.is_signer;
            found.is_writable |= meta.is_writable;
        };

        let mut add_to_non_signers =
            |meta: &solana_instruction::AccountMeta| {
                assert!(!meta.is_signer, "AccountMeta must not be a signer");
                let Some(found) =
                    non_signers.iter_mut().find(|m| m.pubkey == meta.pubkey)
                else {
                    non_signers.push(meta.clone());
                    return;
                };

                found.is_writable |= meta.is_writable;
            };

        for meta in self
            .iter()
            .flat_map(|ix| ix.accounts.iter())
            .filter(|meta| meta.is_signer)
        {
            add_to_signers(meta);
        }

        for ix in self.iter() {
            add_to_non_signers(&solana_instruction::AccountMeta::new_readonly(
                ix.program_id,
                false,
            ));
            for meta in ix.accounts.iter().filter(|meta| !meta.is_signer) {
                let Some(found) =
                    signers.iter_mut().find(|m| m.pubkey == meta.pubkey)
                else {
                    add_to_non_signers(meta);
                    continue;
                };

                found.is_writable |= meta.is_writable;
            }
        }

        if signers.len() + non_signers.len()
            > crate::compact::MAX_PUBKEYS as usize
        {
            panic!(
                "delegate_with_actions supports at most {} unique pubkeys",
                crate::compact::MAX_PUBKEYS
            );
        }

        let index_of = |pk: &solana_address::Address| -> u8 {
            if let Some(index) = signers.iter().position(|s| &s.pubkey == pk) {
                return index as u8;
            }
            signers.len() as u8
                + non_signers.iter().position(|ns| &ns.pubkey == pk).unwrap()
                    as u8
        };

        let compact_instructions: Vec<MaybeEncryptedInstruction> = self
            .into_iter()
            .map(|ix| MaybeEncryptedInstruction {
                program_id: index_of(&ix.program_id),

                accounts: ix
                    .accounts
                    .into_iter()
                    .map(|meta| {
                        let index = index_of(&meta.pubkey);
                        crate::compact::AccountMeta::try_new(
                            index,
                            meta.is_signer,
                            meta.is_writable,
                        )
                        .expect("compact account index must fit in 6 bits")
                        .cleartext()
                    })
                    .collect(),

                data: ix.data.cleartext(),
            })
            .collect();

        PostDelegationActions {
            signers: signers.iter().map(|s| s.pubkey.to_bytes()).collect(),

            non_signers: non_signers
                .into_iter()
                .map(|ns| MaybeEncryptedPubkey::ClearText(ns.pubkey.to_bytes()))
                .collect(),

            instructions: compact_instructions,
        }
    }
}
