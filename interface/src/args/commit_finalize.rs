use wheels::variable_offset_layout;

use crate::compat::Pubkey;

#[derive(Clone, Debug, PartialEq, Eq)]
#[variable_offset_layout(buffer_offset = 1)]
pub struct CommitFinalizeArgs {
    /// Next commit sequence number for this delegated account.
    pub commit_seq: u64,

    /// Account lamports on the ER after this commit.
    pub lamports: u64,

    /// Account owner committed by the ER.
    pub owner: Pubkey,

    /// Whether the validator requests undelegation after this commit.
    pub allow_undelegation: bool,

    /// Stored as `CommitPayloadKind::value()`.
    pub payload_kind: u8,

    /// Keeps the inline payload four-byte aligned for zero-copy diff decoding.
    pub payload_alignment_padding: u8,

    /// Complete account data or an encoded diff, according to `payload_kind`.
    #[flexible = 4]
    pub payload: Vec<u8>,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommitPayloadKind {
    FullData = 0,
    Diff = 1,
}

impl CommitPayloadKind {
    pub const fn value(self) -> u8 {
        self as u8
    }
}
