use serde::{Deserialize, Serialize};
use solana_program::{instruction::Instruction, pubkey::Pubkey};

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
    /// Number of signer pubkeys in the `pubkeys` prefix.
    /// First `signer_count` entries of `pubkeys` are required signers.
    pub signer_count: u8,

    /// Shared pubkey table. Account metas and program IDs reference this table by index.
    pub pubkeys: Vec<Pubkey>,

    /// Instruction payload in compact cleartext or encrypted bytes.
    pub instructions: Instructions,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Instructions {
    /// Compact cleartext instructions.
    ClearText {
        instructions: Vec<compact::Instruction>,
    },

    /// Encrypted compact instruction bytes.
    Encrypted { instructions: Vec<u8> },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DecryptedInstructions {
    /// Sender-provided nonce/salt to randomize ciphertext so identical
    /// plaintext does not always map to identical encrypted bytes.
    pub random_salt: u64,

    /// Decrypted instructions ready for execution.
    pub instructions: Vec<Instruction>,
}
