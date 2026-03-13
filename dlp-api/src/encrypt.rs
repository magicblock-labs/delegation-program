use dlp::args::{
    EncryptedBuffer, MaybeEncryptedAccountMeta, MaybeEncryptedIxData,
    MaybeEncryptedPubkey,
};
use solana_program::{instruction::AccountMeta, pubkey::Pubkey};

use crate::{
    encryption::EncryptionError,
    instruction_builder::{
        Encrypt, EncryptableAccountMeta, EncryptableIxData, EncryptablePubkey,
        PostDelegationInstruction,
    },
};

impl Encrypt for EncryptablePubkey {
    type Output = MaybeEncryptedPubkey;
    type Error = EncryptionError;

    fn encrypt(self, validator: &Pubkey) -> Result<Self::Output, Self::Error> {
        if self.is_encryptable {
            Ok(MaybeEncryptedPubkey::Encrypted(EncryptedBuffer::new(
                crate::encryption::encrypt_ed25519_recipient(
                    self.pubkey.as_array(),
                    validator.as_array(),
                )?,
            )))
        } else {
            Ok(MaybeEncryptedPubkey::ClearText(self.pubkey.to_bytes()))
        }
    }
}

impl Encrypt for EncryptableIxData {
    type Output = MaybeEncryptedIxData;
    type Error = EncryptionError;

    fn encrypt(self, validator: &Pubkey) -> Result<Self::Output, Self::Error> {
        if self.encrypt_begin_offset >= self.data.len() {
            Ok(MaybeEncryptedIxData {
                prefix: self.data,
                suffix: EncryptedBuffer::default(),
            })
        } else {
            Ok(MaybeEncryptedIxData {
                prefix: self.data[0..self.encrypt_begin_offset].into(),
                suffix: EncryptedBuffer::new(
                    crate::encryption::encrypt_ed25519_recipient(
                        &self.data[self.encrypt_begin_offset..],
                        validator.as_array(),
                    )?,
                ),
            })
        }
    }
}

impl Encrypt for dlp::compact::EncryptableAccountMeta {
    type Output = MaybeEncryptedAccountMeta;
    type Error = EncryptionError;

    fn encrypt(self, validator: &Pubkey) -> Result<Self::Output, Self::Error> {
        if self.is_encryptable {
            Ok(MaybeEncryptedAccountMeta::Encrypted(EncryptedBuffer::new(
                crate::encryption::encrypt_ed25519_recipient(
                    &[self.account_meta.to_byte()],
                    validator.as_array(),
                )?,
            )))
        } else {
            Ok(MaybeEncryptedAccountMeta::ClearText(self.account_meta))
        }
    }
}

impl Encrypt for Vec<PostDelegationInstruction> {
    type Output = (dlp::args::PostDelegationActions, Vec<AccountMeta>);
    type Error = EncryptionError;

    fn encrypt(self, validator: &Pubkey) -> Result<Self::Output, Self::Error> {
        use dlp::args::MaybeEncryptedInstruction;

        let mut signers: Vec<AccountMeta> = Vec::new();

        let mut add_to_signers = |meta: &EncryptableAccountMeta| {
            assert!(
                meta.account_meta.is_signer,
                "AccountMeta must be a signer"
            );
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

        for meta in self
            .iter()
            .flat_map(|ix| ix.accounts.iter())
            .filter(|meta| meta.account_meta.is_signer)
        {
            add_to_signers(meta);
        }

        for ix in self.iter() {
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

        if signers.len() + non_signers.len()
            > dlp::compact::MAX_PUBKEYS as usize
        {
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
                    .expect("pubkey must exist in signers or non_signers")
                    as u8
        };

        let compact_instructions: Vec<MaybeEncryptedInstruction> = self
            .into_iter()
            .map(|ix| MaybeEncryptedInstruction {
                program_id: index_of(&ix.program_id.pubkey),

                accounts: ix
                    .accounts
                    .into_iter()
                    .map(|meta| {
                        let index = index_of(&meta.account_meta.pubkey);
                        meta.to_compact(index)
                            .encrypt(validator)
                            .expect("account metadata encryption failed")
                    })
                    .collect(),

                data: ix
                    .data
                    .encrypt(validator)
                    .expect("instruction data encryption failed"),
            })
            .collect();

        Ok((
            dlp::args::PostDelegationActions {
                inserted_signers: 0,
                inserted_non_signers: 0,
                signers: signers.iter().map(|s| s.pubkey.to_bytes()).collect(),

                non_signers: non_signers
                    .into_iter()
                    .map(|ns| {
                        EncryptablePubkey {
                            pubkey: ns.account_meta.pubkey,
                            is_encryptable: ns.is_encryptable,
                        }
                        .encrypt(validator)
                        .expect("pubkey encryption failed")
                    })
                    .collect(),

                instructions: compact_instructions,
            },
            signers,
        ))
    }
}
