use super::DelegateArgs;
use crate::compact;
use borsh::{BorshDeserialize, BorshSerialize};

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct DelegateWithActionsArgs {
    /// Standard delegation parameters.
    pub delegate: DelegateArgs,

    /// Compact post-delegation actions.
    pub actions: PostDelegationActions,
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct PostDelegationActions {
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
