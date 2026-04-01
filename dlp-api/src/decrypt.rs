use dlp::{
    args::{
        MaybeEncryptedAccountMeta, MaybeEncryptedIxData, MaybeEncryptedPubkey,
        PostDelegationActions,
    },
    compact,
};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    signer::Signer,
};
use thiserror::Error;

use crate::encryption::{self, EncryptionError, KEY_LEN};

#[derive(Debug, Error)]
pub enum DecryptError {
    #[error(transparent)]
    DecryptFailed(#[from] EncryptionError),

    #[error("invalid decrypted pubkey length: {0}")]
    InvalidPubkeyLength(usize),

    #[error("invalid decrypted compact account meta length: {0}")]
    InvalidAccountMetaLength(usize),

    #[error("invalid decrypted compact account meta value: {0}")]
    InvalidAccountMetaValue(u8),

    #[error("invalid program_id index {index} for pubkey table len {len}")]
    InvalidProgramIdIndex { index: u8, len: usize },

    #[error("invalid account index {index} for pubkey table len {len}")]
    InvalidAccountIndex { index: u8, len: usize },

    #[error("invalid inserted signer count {inserted} for signers len {len}")]
    InvalidInsertedSignerCount { inserted: u8, len: usize },

    #[error("invalid inserted non-signer count {inserted} for non-signers len {len}")]
    InvalidInsertedNonSignerCount { inserted: u8, len: usize },

    #[error("non-signer (index {index}) cannot be used as signer (valid signer index ranges are {old_signer_range:?} and {new_signer_range:?}, start inclusive and end exclusive)")]
    NonSignerCannotBeSigner {
        index: usize,
        old_signer_range: (usize, usize),
        new_signer_range: (usize, usize),
    },
}

pub trait Decrypt: Sized {
    type Output;

    fn decrypt(
        self,
        recipient_x25519_pubkey: &[u8; KEY_LEN],
        recipient_x25519_secret: &[u8; KEY_LEN],
    ) -> Result<Self::Output, DecryptError>;

    fn decrypt_with_keypair(
        self,
        recipient_keypair: &solana_sdk::signature::Keypair,
    ) -> Result<Self::Output, DecryptError>
    where
        Self: Sized,
    {
        let recipient_x25519_secret =
            encryption::keypair_to_x25519_secret(recipient_keypair)?;
        let recipient_x25519_pubkey = encryption::ed25519_pubkey_to_x25519(
            recipient_keypair.pubkey().as_array(),
        )?;
        self.decrypt(&recipient_x25519_pubkey, &recipient_x25519_secret)
    }
}

impl Decrypt for MaybeEncryptedPubkey {
    type Output = [u8; 32];

    fn decrypt(
        self,
        recipient_x25519_pubkey: &[u8; KEY_LEN],
        recipient_x25519_secret: &[u8; KEY_LEN],
    ) -> Result<Self::Output, DecryptError> {
        match self {
            Self::ClearText(pubkey) => Ok(pubkey),
            Self::Encrypted(buffer) => {
                let plaintext = encryption::decrypt(
                    buffer.as_bytes(),
                    recipient_x25519_pubkey,
                    recipient_x25519_secret,
                )
                .map_err(DecryptError::DecryptFailed)?;
                Self::Output::try_from(plaintext.as_slice()).map_err(|_| {
                    DecryptError::InvalidPubkeyLength(plaintext.len())
                })
            }
        }
    }
}

impl Decrypt for MaybeEncryptedAccountMeta {
    type Output = compact::AccountMeta;

    fn decrypt(
        self,
        recipient_x25519_pubkey: &[u8; KEY_LEN],
        recipient_x25519_secret: &[u8; KEY_LEN],
    ) -> Result<Self::Output, DecryptError> {
        match self {
            Self::ClearText(account_meta) => Ok(account_meta),
            Self::Encrypted(buffer) => {
                let plaintext = encryption::decrypt(
                    buffer.as_bytes(),
                    recipient_x25519_pubkey,
                    recipient_x25519_secret,
                )
                .map_err(DecryptError::DecryptFailed)?;
                if plaintext.len() != 1 {
                    return Err(DecryptError::InvalidAccountMetaLength(
                        plaintext.len(),
                    ));
                }
                compact::AccountMeta::from_byte(plaintext[0])
                    .ok_or(DecryptError::InvalidAccountMetaValue(plaintext[0]))
            }
        }
    }
}

impl Decrypt for MaybeEncryptedIxData {
    type Output = Vec<u8>;

    fn decrypt(
        self,
        recipient_x25519_pubkey: &[u8; KEY_LEN],
        recipient_x25519_secret: &[u8; KEY_LEN],
    ) -> Result<Self::Output, DecryptError> {
        let mut data = self.prefix;
        if !self.suffix.as_bytes().is_empty() {
            let suffix = encryption::decrypt(
                self.suffix.as_bytes(),
                recipient_x25519_pubkey,
                recipient_x25519_secret,
            )
            .map_err(DecryptError::DecryptFailed)?;
            data.extend_from_slice(&suffix);
        }
        Ok(data)
    }
}

impl Decrypt for PostDelegationActions {
    type Output = Vec<Instruction>;

    /// This function decrypts PostDelegationActions as well as
    /// validates it, matching the expected signers with the AccountMetas.
    fn decrypt(
        self,
        recipient_x25519_pubkey: &[u8; KEY_LEN],
        recipient_x25519_secret: &[u8; KEY_LEN],
    ) -> Result<Self::Output, DecryptError> {
        let actions = self;
        let inserted_signers = actions.inserted_signers as usize;
        let inserted_non_signers = actions.inserted_non_signers as usize;
        let signers_count = actions.signers.len();
        let non_signers_count = actions.non_signers.len();

        if inserted_signers > signers_count {
            return Err(DecryptError::InvalidInsertedSignerCount {
                inserted: actions.inserted_signers,
                len: signers_count,
            });
        }
        if inserted_non_signers > non_signers_count {
            return Err(DecryptError::InvalidInsertedNonSignerCount {
                inserted: actions.inserted_non_signers,
                len: non_signers_count,
            });
        }

        // Rebuild the lookup table in the same order used during compact
        // index assignment (see compact module):
        // [old signers][old non-signers][new signers][new non-signers]
        let pubkeys = {
            let mut old_signers = actions.signers;
            let new_signers = old_signers.split_off(inserted_signers);

            let mut old_non_signers = actions
                .non_signers
                .iter()
                .map(|non_signer| {
                    Ok(non_signer.clone().decrypt(
                        recipient_x25519_pubkey,
                        recipient_x25519_secret,
                    )?)
                })
                .collect::<Result<Vec<_>, DecryptError>>()?;

            let new_non_signers =
                old_non_signers.split_off(inserted_non_signers);

            [old_signers, old_non_signers, new_signers, new_non_signers]
                .concat()
        };
        let inserted_total = inserted_signers + inserted_non_signers;
        let new_signers_count = signers_count - inserted_signers;

        let old_signer_range = (0, inserted_signers);
        let new_signer_range =
            (inserted_total, inserted_total + new_signers_count);

        let is_signer_idx = |idx: usize| {
            (old_signer_range.0..old_signer_range.1).contains(&idx)
                || (new_signer_range.0..new_signer_range.1).contains(&idx)
        };

        let instructions = actions
            .instructions
            .into_iter()
            .map(|ix| {
                Ok(Instruction {
                    program_id: pubkeys
                        .get(ix.program_id as usize)
                        .copied()
                        .ok_or(DecryptError::InvalidProgramIdIndex {
                            index: ix.program_id,
                            len: pubkeys.len(),
                        })?
                        .into(),

                    accounts: ix
                        .accounts
                        .into_iter()
                        .map(|maybe_compact_meta| {
                            let compact_meta = maybe_compact_meta.decrypt(
                                recipient_x25519_pubkey,
                                recipient_x25519_secret,
                            )?;
                            let idx = compact_meta.key() as usize;

                            if compact_meta.is_signer() && !is_signer_idx(idx) {
                                return Err(
                                    DecryptError::NonSignerCannotBeSigner {
                                        index: idx,
                                        old_signer_range,
                                        new_signer_range,
                                    },
                                );
                            }

                            let account_pubkey = pubkeys
                                .get(idx)
                                .copied()
                                .ok_or(DecryptError::InvalidAccountIndex {
                                    index: compact_meta.key(),
                                    len: pubkeys.len(),
                                })?;
                            Ok(AccountMeta {
                                pubkey: account_pubkey.into(),
                                is_signer: compact_meta.is_signer(),
                                is_writable: compact_meta.is_writable(),
                            })
                        })
                        .collect::<Result<Vec<_>, DecryptError>>()?,

                    data: ix.data.decrypt(
                        recipient_x25519_pubkey,
                        recipient_x25519_secret,
                    )?,
                })
            })
            .collect::<Result<Vec<_>, DecryptError>>()?;

        Ok(instructions)
    }
}

#[cfg(test)]
mod tests {
    use solana_program::instruction::AccountMeta;
    use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer};

    use super::*;
    use crate::instruction_builder::{
        Encrypt, Encryptable, EncryptableFrom, PostDelegationInstruction,
    };

    #[test]
    fn test_post_delegation_actions_decrypt_roundtrip() {
        let validator = Keypair::new();
        let signer = Pubkey::new_unique();
        let nonsigner = Pubkey::new_unique();
        let program_id = Pubkey::new_unique();

        let instructions = vec![PostDelegationInstruction {
            program_id: program_id.cleartext(),
            accounts: vec![
                AccountMeta::new_readonly(signer, true).cleartext(),
                AccountMeta::new_readonly(nonsigner, false).encrypted(),
            ],
            data: vec![1, 2, 3, 4].encrypted_from(2),
        }];

        let (actions, signers) = instructions
            .encrypt(&validator.pubkey())
            .expect("post-delegation actions encryption failed");

        assert_eq!(signers, vec![AccountMeta::new_readonly(signer, true)]);

        let decrypted = actions.decrypt_with_keypair(&validator).unwrap();

        assert_eq!(
            decrypted,
            vec![Instruction {
                program_id,
                accounts: vec![
                    AccountMeta::new_readonly(signer, true),
                    AccountMeta::new_readonly(nonsigner, false)
                ],
                data: vec![1, 2, 3, 4]
            }]
        );
    }
}
