use dlp::args::{
    EncryptedBuffer, MaybeEncryptedAccountMeta, MaybeEncryptedIxData,
};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

use crate::instruction_builder::{Encryptable, EncryptableFrom};

/// PostDelegationInstruction + Encryptable
pub struct PostDelegationInstruction {
    pub program_id: EncryptablePubkey,
    pub accounts: Vec<EncryptableAccountMeta>,
    pub data: EncryptableIxData,
}

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

/// EncryptableAccountMeta + Encryptable
#[derive(Clone, Debug)]
pub struct EncryptableAccountMeta {
    pub account_meta: AccountMeta,
    pub is_encryptable: bool,
}

impl EncryptableAccountMeta {
    pub fn encrypt_with_index(
        self,
        validator: &Option<Pubkey>,
        index: u8,
    ) -> MaybeEncryptedAccountMeta {
        if self.is_encryptable {
            let validator = validator.expect("");
            MaybeEncryptedAccountMeta::Encrypted(EncryptedBuffer::new(
                crate::encryption::encrypt_ed25519_recipient(
                    self.account_meta.pubkey.as_array(),
                    validator.as_array(),
                )
                .expect(""),
            ))
        } else {
            MaybeEncryptedAccountMeta::ClearText(
                dlp::compact::AccountMeta::try_new(
                    index,
                    false,
                    self.account_meta.is_writable,
                )
                .expect("compact account index must fit in 6 bits"),
            )
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

impl EncryptableIxData {
    pub fn encrypt(self, validator: &Option<Pubkey>) -> MaybeEncryptedIxData {
        if self.encrypt_begin_offset >= self.data.len() {
            MaybeEncryptedIxData {
                prefix: self.data,
                suffix: EncryptedBuffer::default(),
            }
        } else {
            let validator = validator.expect("");
            MaybeEncryptedIxData {
                prefix: self.data[0..self.encrypt_begin_offset].into(),
                suffix: EncryptedBuffer::new(
                    crate::encryption::encrypt_ed25519_recipient(
                        &self.data[self.encrypt_begin_offset..],
                        validator.as_array(),
                    )
                    .expect(""),
                ),
            }
        }
    }
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
