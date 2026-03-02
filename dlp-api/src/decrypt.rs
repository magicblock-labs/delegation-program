use dlp::args::{
    MaybeEncryptedAccountMeta, MaybeEncryptedIxData, MaybeEncryptedPubkey,
    PostDelegationActions,
};
use dlp::compact;
use solana_program::pubkey::Pubkey;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::signer::Signer;
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
    type Output = Pubkey;

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
                Pubkey::try_from(plaintext.as_slice()).map_err(|_| {
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

    fn decrypt(
        self,
        recipient_x25519_pubkey: &[u8; KEY_LEN],
        recipient_x25519_secret: &[u8; KEY_LEN],
    ) -> Result<Self::Output, DecryptError> {
        let actions = self;

        let pubkeys = {
            let mut pubkeys = actions.signers;
            for non_signer in actions.non_signers {
                pubkeys.push(non_signer.decrypt(
                    recipient_x25519_pubkey,
                    recipient_x25519_secret,
                )?);
            }
            pubkeys
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
                        })?,

                    accounts: ix
                        .accounts
                        .into_iter()
                        .map(|maybe_compact_meta| {
                            let compact_meta = maybe_compact_meta.decrypt(
                                recipient_x25519_pubkey,
                                recipient_x25519_secret,
                            )?;
                            let account_pubkey = pubkeys
                                .get(compact_meta.key() as usize)
                                .copied()
                                .ok_or(DecryptError::InvalidAccountIndex {
                                    index: compact_meta.key(),
                                    len: pubkeys.len(),
                                })?;

                            Ok(AccountMeta {
                                pubkey: account_pubkey,
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
    use super::*;
    use crate::instruction_builder::{
        create_post_delegation_actions, Encryptable, EncryptableFrom,
        PostDelegationInstruction,
    };
    use solana_program::instruction::AccountMeta;
    use solana_sdk::{signature::Keypair, signer::Signer};

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

        let (actions, signers) = create_post_delegation_actions(
            instructions,
            Some(validator.pubkey()),
        );

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
