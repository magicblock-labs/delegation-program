use dlp::args::{
    EncryptedBuffer, MaybeEncryptedAccountMeta, MaybeEncryptedIxData,
    MaybeEncryptedPubkey,
};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

use crate::{
    encryption::EncryptionError,
    instruction_builder::{Encrypt, Encryptable, EncryptableFrom},
};

/// PostDelegationInstruction + Encryptable
pub struct PostDelegationInstruction {
    pub program_id: EncryptablePubkey,
    pub accounts: Vec<EncryptableAccountMeta>,
    pub data: EncryptableIxData,
}

/// Instruction is never encrypted and only its parts are encrypted;
/// and this Encryptable implementation is a shorthand for calling
/// encrypted() and encrypted_from(0) on all its parts.
impl Encryptable for Instruction {
    type Output = PostDelegationInstruction;
    fn with_encryption(self, encrypt: bool) -> Self::Output {
        if encrypt {
            PostDelegationInstruction {
                program_id: self.program_id.encrypted(),
                accounts: self
                    .accounts
                    .into_iter()
                    .map(|m| m.encrypted())
                    .collect(),
                data: self.data.encrypted_from(0),
            }
        } else {
            PostDelegationInstruction {
                program_id: self.program_id.cleartext(),
                accounts: self
                    .accounts
                    .into_iter()
                    .map(|m| m.cleartext())
                    .collect(),
                data: self.data.encrypted_from(usize::MAX),
            }
        }
    }
}

/// Instruction is never encrypted and only its parts are encrypted;
/// and this Encryptable implementation is a shorthand for calling
/// encrypted() and encrypted_from(offset) on all its parts.
impl EncryptableFrom for Instruction {
    type Output = PostDelegationInstruction;
    fn encrypted_from(self, offset: usize) -> Self::Output {
        PostDelegationInstruction {
            program_id: self.program_id.encrypted(),
            accounts: self
                .accounts
                .into_iter()
                .map(|m| m.encrypted())
                .collect(),
            data: self.data.encrypted_from(offset),
        }
    }
}

/// EncryptablePubkey + Encryptable
#[derive(Clone, Debug)]
pub struct EncryptablePubkey {
    pub pubkey: Pubkey,
    pub is_encryptable: bool,
}

impl Encryptable for Pubkey {
    type Output = EncryptablePubkey;
    fn with_encryption(self, encrypt: bool) -> Self::Output {
        EncryptablePubkey {
            pubkey: self,
            is_encryptable: encrypt,
        }
    }
}

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

/// EncryptableAccountMeta + Encryptable
// NOTE: This type is not encrypted directly. We first convert it to its
// compact::EncryptableAccountMeta which gets encrypted.
#[derive(Clone, Debug)]
pub struct EncryptableAccountMeta {
    pub account_meta: AccountMeta,
    pub is_encryptable: bool,
}

impl EncryptableAccountMeta {
    pub fn to_compact(self, index: u8) -> dlp::compact::EncryptableAccountMeta {
        dlp::compact::EncryptableAccountMeta {
            account_meta: dlp::compact::AccountMeta::try_new(
                index,
                self.account_meta.is_signer,
                self.account_meta.is_writable,
            )
            .expect("compact account index must fit in 6 bits"),
            is_encryptable: self.is_encryptable,
        }
    }
}

impl Encryptable for AccountMeta {
    type Output = EncryptableAccountMeta;
    fn with_encryption(self, encrypt: bool) -> Self::Output {
        EncryptableAccountMeta {
            account_meta: self,
            is_encryptable: encrypt,
        }
    }
}

/// EncryptableIxData + EncryptableFrom
#[derive(Clone, Debug)]
pub struct EncryptableIxData {
    pub data: Vec<u8>,

    /// [0, encrypt_offset) is cleartext and [encrypt_offset, len) is encrypted
    pub encrypt_begin_offset: usize,
}

impl EncryptableFrom for Vec<u8> {
    type Output = EncryptableIxData;
    fn encrypted_from(self, offset: usize) -> Self::Output {
        EncryptableIxData {
            data: self,
            encrypt_begin_offset: offset,
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
