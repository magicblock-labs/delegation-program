use serde::{Deserialize, Serialize};
use solana_program::pubkey::Pubkey;
use super::DelegateArgs;
use crate::compact;

#[derive(Debug, Serialize, Deserialize)]
pub struct DelegateWithActionsArgs {
    /// Standard delegation parameters.
    pub delegate: DelegateArgs,

    /// Compact post-delegation actions.
    pub actions: PostDelegationActions,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PostDelegationActions {
    pub signers: Vec<Pubkey>,

    pub non_signers: Vec<MaybeEncryptedAccountMeta>,

    pub instructions: Vec<MaybeEncryptedInstruction>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MaybeEncryptedInstruction {
    pub program_id: u8,

    pub accounts: Vec<MaybeEncryptedAccountMeta>,

    pub data: MaybeEncryptedIxData,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MaybeEncryptedPubkey {
    ClearText(Pubkey),
    Encrypted(EncryptedBuffer),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MaybeEncryptedAccountMeta {
    ClearText(compact::AccountMeta),
    Encrypted(EncryptedBuffer),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MaybeEncryptedIxData {
    pub prefix: Vec<u8>,
    pub suffix: EncryptedBuffer,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
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
