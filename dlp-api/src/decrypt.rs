use dlp::args::{
    MaybeEncryptedAccountMeta, MaybeEncryptedIxData, MaybeEncryptedPubkey,
};
use dlp::compact;
use solana_program::pubkey::Pubkey;
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

fn decrypt_bytes(
    encrypted: &[u8],
    recipient_x25519_pubkey: &[u8; KEY_LEN],
    recipient_x25519_secret: &[u8; KEY_LEN],
) -> Result<Vec<u8>, DecryptError> {
    encryption::decrypt(
        encrypted,
        recipient_x25519_pubkey,
        recipient_x25519_secret,
    )
    .map_err(DecryptError::DecryptFailed)
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
                let plaintext = decrypt_bytes(
                    buffer.as_bytes(),
                    recipient_x25519_pubkey,
                    recipient_x25519_secret,
                )?;
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
                let plaintext = decrypt_bytes(
                    buffer.as_bytes(),
                    recipient_x25519_pubkey,
                    recipient_x25519_secret,
                )?;
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
            let suffix = decrypt_bytes(
                self.suffix.as_bytes(),
                recipient_x25519_pubkey,
                recipient_x25519_secret,
            )?;
            data.extend_from_slice(&suffix);
        }
        Ok(data)
    }
}
