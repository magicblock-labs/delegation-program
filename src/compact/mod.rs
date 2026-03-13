mod account_meta;
mod instruction;

pub use account_meta::*;
pub use instruction::*;
use pinocchio::Address;

use crate::args::{
    EncryptedBuffer, MaybeEncryptedInstruction, MaybeEncryptedIxData,
    MaybeEncryptedPubkey, PostDelegationActions,
};

pub trait ClearText: Sized {
    type Output;

    fn cleartext(self) -> Self::Output;
}

pub trait ClearTextWithInsertable: Sized {
    type Output;

    fn cleartext_with_insertable(
        self,
        insertable: PostDelegationActions,
        insert_before_index: usize,
    ) -> Self::Output;
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
                + non_signers
                    .iter()
                    .position(|ns| &ns.pubkey == pk)
                    .expect("pubkey must exist in signers or non_signers")
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
            inserted_signers: 0,
            inserted_non_signers: 0,

            signers: signers.iter().map(|s| s.pubkey.to_bytes()).collect(),

            non_signers: non_signers
                .into_iter()
                .map(|ns| MaybeEncryptedPubkey::ClearText(ns.pubkey.to_bytes()))
                .collect(),

            instructions: compact_instructions,
        }
    }
}

impl ClearTextWithInsertable for Vec<solana_instruction::Instruction> {
    type Output = PostDelegationActions;
    fn cleartext_with_insertable(
        self,
        insertable: PostDelegationActions,
        insert_before_index: usize,
    ) -> Self::Output {
        assert!(
            insertable.inserted_signers == 0,
            "PostDelegationActions does not support multiple merge/insert"
        );
        assert!(
            insertable.inserted_non_signers == 0,
            "PostDelegationActions does not support multiple merge/insert"
        );

        // add keys from actions (pre-encrypted instructions)
        let mut skipable_pubkeys: Vec<Option<Address>> = vec![];
        {
            for signer in insertable.signers.iter() {
                skipable_pubkeys.push(Some((*signer).into()));
            }
            for non_signer in insertable.non_signers.iter() {
                if let MaybeEncryptedPubkey::ClearText(non_signer) = non_signer
                {
                    skipable_pubkeys.push(Some((*non_signer).into()));
                } else {
                    // Note that None is added to the list, to mark that this slot is encrypted but
                    // the index is already taken so that the index in referred by insertable.instructions
                    // is maintained/calculatable.
                    skipable_pubkeys.push(None);
                }
            }
        }

        let mut signers: Vec<solana_instruction::AccountMeta> = Vec::new();
        let mut non_signers: Vec<solana_instruction::AccountMeta> = Vec::new();

        let mut add_to_signers = |meta: &solana_instruction::AccountMeta| {
            if skipable_pubkeys.contains(&Some(meta.pubkey)) {
                return;
            }

            assert!(meta.is_signer, "AccountMeta must be a signer");
            let Some(found) =
                signers.iter_mut().find(|m| m.pubkey == meta.pubkey)
            else {
                signers.push(meta.clone());
                return;
            };

            found.is_writable |= meta.is_writable;
        };

        let mut add_to_non_signers =
            |meta: &solana_instruction::AccountMeta| {
                if skipable_pubkeys.contains(&Some(meta.pubkey)) {
                    return;
                }

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

        let old_signers_len = insertable.signers.len();
        let old_non_signers_len = insertable.non_signers.len();
        let old_total = old_signers_len + old_non_signers_len;

        let index_of = |pk: &solana_address::Address| -> u8 {
            // The final list will be this as per PostDelegationActions:
            //
            //  [insertable.signers..., new.signers..., insertable.non_signers..., new.non_signers...]
            //
            // However, the final list will invalidate the indices (of non-signers) referred to
            // by insertable.instructions, though indices of signers will continue to be correct.
            //
            // To deal with that, we need to compute the indices of newly added pubkeys
            // differently, accordingly to this imagined list:
            //
            //  [insertable.signers..., insertable.non_signers..., new.signers..., new.non_signers...]
            //
            // That means, if a key is found in skipable_pubkeys, its index will be returned as it
            // is. Else, we'll add `old_total` to the index computed for the following list:
            //
            //  [new.signers..., new.non_signers...]
            //
            if let Some(index) = skipable_pubkeys
                .iter()
                .position(|pubkey| pubkey == &Some(*pk))
            {
                return index as u8;
            }

            if let Some(index) = signers.iter().position(|s| &s.pubkey == pk) {
                return (old_total + index) as u8;
            }
            (old_total
                + signers.len()
                + non_signers.iter().position(|ns| &ns.pubkey == pk).unwrap())
                as u8
        };

        let mut compact_instructions: Vec<MaybeEncryptedInstruction> = self
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

        // Merge all parts now
        let mut rv = insertable;
        rv.inserted_signers = old_signers_len as u8;
        rv.inserted_non_signers = old_non_signers_len as u8;

        rv.signers.extend_from_slice(
            &signers
                .iter()
                .map(|s| s.pubkey.to_bytes())
                .collect::<Vec<_>>(),
        );
        rv.non_signers.extend_from_slice(
            &non_signers
                .iter()
                .map(|ns| MaybeEncryptedPubkey::ClearText(ns.pubkey.to_bytes()))
                .collect::<Vec<_>>(),
        );

        if insert_before_index <= compact_instructions.len() {
            compact_instructions.splice(
                insert_before_index..insert_before_index,
                rv.instructions,
            );
        } else {
            compact_instructions.extend_from_slice(&rv.instructions);
        }

        rv.instructions = compact_instructions;

        rv
    }
}

#[cfg(test)]
mod tests {
    use solana_instruction::{AccountMeta, Instruction};
    use solana_pubkey::Pubkey;

    use super::*;
    use crate::args::MaybeEncryptedAccountMeta;

    fn pk(byte: u8) -> Pubkey {
        Pubkey::new_from_array([byte; 32])
    }

    fn assert_cleartext_meta(
        meta: &MaybeEncryptedAccountMeta,
        expected_index: u8,
        expected_signer: bool,
    ) {
        let MaybeEncryptedAccountMeta::ClearText(meta) = meta else {
            panic!("expected cleartext account meta");
        };
        assert_eq!(meta.key(), expected_index);
        assert_eq!(meta.is_signer(), expected_signer);
    }

    #[test]
    fn test_cleartext_with_insertable_indices() {
        let s1 = pk(1);
        let s2 = pk(2);
        let n1 = pk(3);
        let n2 = pk(4);
        let n3 = pk(5);

        let insertable = PostDelegationActions {
            inserted_signers: 0,
            inserted_non_signers: 0,
            signers: vec![s1.to_bytes(), s2.to_bytes()],
            non_signers: vec![
                MaybeEncryptedPubkey::ClearText(n1.to_bytes()),
                MaybeEncryptedPubkey::ClearText(n2.to_bytes()),
                MaybeEncryptedPubkey::Encrypted(EncryptedBuffer::new(
                    n3.to_bytes().into(),
                )),
            ],
            instructions: vec![MaybeEncryptedInstruction {
                program_id: 0,
                accounts: vec![],
                data: MaybeEncryptedIxData {
                    prefix: vec![],
                    suffix: EncryptedBuffer::new(vec![]),
                },
            }],
        };

        let ns1 = pk(6);
        let nn1 = pk(7);
        let program_id = pk(8);

        let ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new_readonly(s1, true), // reuse old key
                AccountMeta::new_readonly(ns1, true),
                AccountMeta::new_readonly(nn1, false),
                AccountMeta::new_readonly(n3, false), // reuse old key but encrypted
            ],
            data: vec![1, 2, 3],
        };

        let actions = vec![ix].cleartext_with_insertable(insertable, 1);

        assert_eq!(actions.inserted_signers, 2);
        assert_eq!(actions.inserted_non_signers, 3); // even though 1 is encrypted

        assert_eq!(actions.signers.len(), 3);
        assert_eq!(actions.non_signers.len(), 5 + 1); // n3 is inserted again

        assert_eq!(
            actions.signers,
            vec![s1.to_bytes(), s2.to_bytes(), ns1.to_bytes()]
        );
        assert_eq!(
            actions.non_signers,
            vec![
                MaybeEncryptedPubkey::ClearText(n1.to_bytes()),
                MaybeEncryptedPubkey::ClearText(n2.to_bytes()),
                MaybeEncryptedPubkey::Encrypted(EncryptedBuffer::new(
                    n3.to_bytes().into(),
                )),
                MaybeEncryptedPubkey::ClearText(program_id.to_bytes()),
                MaybeEncryptedPubkey::ClearText(nn1.to_bytes()),
                MaybeEncryptedPubkey::ClearText(n3.to_bytes()),
            ]
        );

        assert_eq!(actions.instructions.len(), 2);
        let new_ix = &actions.instructions[0];
        assert_eq!(new_ix.program_id, 6);
        assert_eq!(new_ix.accounts.len(), 4);

        assert_cleartext_meta(&new_ix.accounts[0], 0, true);
        assert_cleartext_meta(&new_ix.accounts[1], 5, true);
        assert_cleartext_meta(&new_ix.accounts[2], 7, false);
        assert_cleartext_meta(&new_ix.accounts[3], 8, false);
    }
}
