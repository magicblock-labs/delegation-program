use borsh::{BorshDeserialize, BorshSerialize};

use super::DelegateArgs;
use crate::compact;

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct DelegateWithActionsArgs {
    /// Standard delegation parameters.
    pub delegate: DelegateArgs,

    /// Compact post-delegation actions.
    pub actions: PostDelegationActions,
}

///
/// This struct is used both as instruction args and as persisted state: it is received as
/// instruction data and then stored in the delegation-record account immediately after
/// DelegationRecord.
///
/// In the basic form, PostDelegationActions is constructed from Vec<Instruction>. In that case,
/// both inserted_signers and inserted_non_signers are zero, and the pubkey indices used by
/// compact::AccountMeta are computed from this imagined pubkey list:
///
///   [signers.., non_signers..]
///
/// That is, the first non-signer pubkey index is signers.len().
///
/// In the advanced form, an existing PostDelegationActions value (usually provided by an off-chain
/// client) is combined with Vec<Instruction> (usually constructed on-chain) to produce a merged
/// PostDelegationActions via ClearTextWithInsertable::cleartext_with_insertable, which allows the
/// existing actions to be inserted at a specific instruction index. In this case, both inserted_signers
/// and inserted_non_signers may be non-zero, and the imagined pubkey list takes this form:
///
///   [old_signers.., old_non_signers.., new_signers.., new_non_signers..]
///
/// That is, the first "new signer" comes after the old keys (old_signers + old_non_signers). Also,
/// the `signers` field is created from:
///
///  [old_signers.., new_signers..]
///
/// and `non-signers` field is created from:
///
///  [old_non_signers.., new_non_signers..]
///
///
#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct PostDelegationActions {
    pub inserted_signers: u8,

    pub inserted_non_signers: u8,

    pub signers: Vec<[u8; 32]>,

    pub non_signers: Vec<MaybeEncryptedPubkey>,

    pub instructions: Vec<MaybeEncryptedInstruction>,
}

#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
pub struct MaybeEncryptedInstruction {
    pub program_id: u8,

    pub accounts: Vec<MaybeEncryptedAccountMeta>,

    pub data: MaybeEncryptedIxData,
}

#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
#[cfg_attr(test, derive(PartialEq))]
pub enum MaybeEncryptedPubkey {
    ClearText([u8; 32]),
    Encrypted(EncryptedBuffer),
}

impl From<[u8; 32]> for MaybeEncryptedPubkey {
    fn from(pubkey: [u8; 32]) -> Self {
        Self::ClearText(pubkey)
    }
}

impl From<Vec<u8>> for MaybeEncryptedPubkey {
    fn from(bytes: Vec<u8>) -> Self {
        Self::Encrypted(bytes.into())
    }
}

#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
pub enum MaybeEncryptedAccountMeta {
    ClearText(compact::AccountMeta),
    Encrypted(EncryptedBuffer),
}

impl From<compact::AccountMeta> for MaybeEncryptedAccountMeta {
    fn from(account_meta: compact::AccountMeta) -> Self {
        Self::ClearText(account_meta)
    }
}

impl From<Vec<u8>> for MaybeEncryptedAccountMeta {
    fn from(bytes: Vec<u8>) -> Self {
        Self::Encrypted(bytes.into())
    }
}

#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
pub struct MaybeEncryptedIxData {
    pub prefix: Vec<u8>,
    pub suffix: EncryptedBuffer,
}

#[derive(Clone, Debug, Default, BorshSerialize, BorshDeserialize)]
#[cfg_attr(test, derive(PartialEq))]
pub struct EncryptedBuffer(Vec<u8>);

impl EncryptedBuffer {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn into_inner(self) -> Vec<u8> {
        self.0
    }
}

impl From<Vec<u8>> for EncryptedBuffer {
    fn from(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }
}
