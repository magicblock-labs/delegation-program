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
            encrypt_begin_offset: offset.min(self.len()),
            data: self,
        }
    }
}

///
/// PostDelegationInstruction {
///   program_id: pubkey
///   accounts: Vec<AccountMetaCompact>,
///   encrypted_accounts: EncryptedVec<AccountMetaCompact>,
///   data: Vec<u8>
///   encryptedData: Vec<u8>
/// }
///
#[test]
fn dev_experience() {
    const USDC_SCALE: u64 = 1000_000;

    use solana_program::{instruction::AccountMeta, pubkey::Pubkey};
    use spl_token::instruction::TokenInstruction;

    let sender = Pubkey::new_unique();
    let recipient = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let amount: u64 = 100 * USDC_SCALE; // 100 USDC with 6 decimals

    let regular_transfer_ix = Instruction {
        program_id: spl_token::id(),
        accounts: vec![
            AccountMeta::new(sender, false),
            AccountMeta::new(recipient, false),
            AccountMeta::new_readonly(authority, true),
        ],
        data: TokenInstruction::Transfer { amount }.pack(),
    };

    // Use:
    //  - encrypted() and encrypted_from() to make parts private
    //  - cleartext() for public
    let private_transfer_ix = PostDelegationInstruction {
        program_id: spl_token::id().cleartext(),
        accounts: vec![
            AccountMeta::new(sender, false).cleartext(),
            AccountMeta::new(recipient, false).encrypted(),
            AccountMeta::new_readonly(authority, true).cleartext(),
        ],
        data: TokenInstruction::Transfer { amount }
            .pack()
            .encrypted_from(1),
    };

    assert_eq!(regular_transfer_ix.program_id, spl_token::id());
    assert_eq!(private_transfer_ix.program_id.pubkey, spl_token::id());
    assert!(private_transfer_ix.accounts[1].is_encryptable);
    assert_eq!(private_transfer_ix.data.encrypt_begin_offset, 1);
}
