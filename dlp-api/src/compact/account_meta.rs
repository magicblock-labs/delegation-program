use borsh::{
    maybestd::io::{Error, Read, Write},
    BorshDeserialize, BorshSerialize,
};
use serde::{Deserialize, Serialize};

use crate::{args::MaybeEncryptedAccountMeta, compact::ClearText};

const ACCOUNT_INDEX_MASK: u8 = 0b0011_1111;
const SIGNER_MASK: u8 = 0b0100_0000;
const WRITABLE_MASK: u8 = 0b1000_0000;

///
/// MAX_PUBKEYS = 64
///
pub const MAX_PUBKEYS: u8 = ACCOUNT_INDEX_MASK + 1;

/// Compact account meta packed into one byte.
/// Bits `0..=5` encode the pubkey-table index (`0..MAX_PUBKEYS-1`).
/// Bit `6` is `is_signer`, and bit `7` is `is_writable`.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct AccountMeta(u8);

impl BorshSerialize for AccountMeta {
    fn serialize<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        BorshSerialize::serialize(&self.0, writer)
    }
}

impl BorshDeserialize for AccountMeta {
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self, Error> {
        let value = u8::deserialize_reader(reader)?;
        Ok(Self(value))
    }
}

impl AccountMeta {
    pub fn new(index: u8, is_signer: bool) -> Self {
        Self::try_new(index, is_signer, true).expect("index is out of range")
    }
    pub fn new_readonly(index: u8, is_signer: bool) -> Self {
        Self::try_new(index, is_signer, false).expect("index is out of range")
    }

    pub fn try_new(
        index: u8,
        is_signer: bool,
        is_writable: bool,
    ) -> Option<Self> {
        if index >= MAX_PUBKEYS {
            return None;
        }
        let mut packed = index;
        if is_signer {
            packed |= SIGNER_MASK;
        }
        if is_writable {
            packed |= WRITABLE_MASK;
        }
        Some(Self(packed))
    }

    pub fn key(self) -> u8 {
        self.0 & ACCOUNT_INDEX_MASK
    }

    pub fn is_signer(self) -> bool {
        (self.0 & SIGNER_MASK) != 0
    }

    pub fn is_writable(self) -> bool {
        (self.0 & WRITABLE_MASK) != 0
    }

    pub fn set_index(&mut self, new_index: u8) {
        *self = Self::try_new(new_index, self.is_signer(), self.is_writable())
            .expect("index is out of range");
    }

    pub fn to_byte(self) -> u8 {
        self.0
    }

    pub fn from_byte(value: u8) -> Option<Self> {
        Self::try_new(
            value & ACCOUNT_INDEX_MASK,
            (value & SIGNER_MASK) != 0,
            (value & WRITABLE_MASK) != 0,
        )
    }
}

impl ClearText for AccountMeta {
    type Output = MaybeEncryptedAccountMeta;

    fn cleartext(self) -> Self::Output {
        MaybeEncryptedAccountMeta::ClearText(self)
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct EncryptableAccountMeta {
    pub account_meta: AccountMeta,
    pub is_encryptable: bool,
}
