use wheels::variable_offset_layout;

use crate::compat::Pubkey;

#[derive(Clone, Debug, PartialEq, Eq)]
#[variable_offset_layout(buffer_offset = 1)]
pub struct CommitFinalizeFromBufferArgs {
    /// Next commit sequence number for this delegated account and its StateBuffer.
    pub commit_seq: u64,

    /// Account lamports on the ER after this commit.
    pub lamports: u64,

    /// Account owner committed by the ER.
    pub owner: Pubkey,

    /// Whether the validator requests undelegation after this commit.
    pub allow_undelegation: bool,

    /// Stored as `CommitPayloadKind::value()`.
    pub payload_kind: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[variable_offset_layout(buffer_offset = 1)]
pub struct PrepareCommitFinalizeArgs {
    /// Next commit sequence number whose finalization needs this account size.
    pub commit_seq: u64,

    /// Final delegated-account data length.
    pub target_data_len: u32,
}
