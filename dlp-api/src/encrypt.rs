use dlp::args::{EncryptedBuffer, MaybeEncryptedAccountMeta, MaybeEncryptedIxData, MaybeEncryptedPubkey};
use solana_program::pubkey::Pubkey;

use crate::{
    encryption::EncryptionError,
    instruction_builder::{Encrypt, EncryptableIxData, EncryptablePubkey},
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
            Ok(MaybeEncryptedPubkey::ClearText(self.pubkey))
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
