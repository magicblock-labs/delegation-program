use wheels::fixed_offset_layout;

use crate::compat::Pubkey;

pub const STATE_BUFFER_MAX_TOTAL_LEN: u32 = 10 * 1024 * 1024;
pub const STATE_BUFFER_MAX_ACCOUNT_GROWTH: usize = 10_240;

/// PDA: `["state-buffer", account, commit_id, authority]`.
/// Created by `WriteStateBuffer`.
/// Closed by `CloseTerminalAccounts` after finalize, cancel, or expiry.
///
/// Fixed header for full account data uploaded before `PostCommitment` or
/// `RaiseChallenge`.
/// Raw account bytes start immediately after this header in the same account.
#[derive(Clone, Debug, PartialEq, Eq)]
#[fixed_offset_layout(buffer_offset = 0)]
pub struct StateBuffer {
    /// Account type marker.
    pub discriminator: [u8; 8],

    /// Writer that owns this opened state.
    ///
    /// This is the operator identity for an operator commitment buffer, or the
    /// challenger identity for a challenger dispute buffer.
    pub authority: Pubkey,

    /// Delegated account whose state bytes are stored after this header.
    pub account_pubkey: Pubkey,

    /// Flow-specific nonce that identifies this opened state.
    pub commit_id: u64,

    /// Hash of the raw uploaded account data. Zero until finalized.
    pub data_hash: [u8; 32],

    /// Expected total byte length of the raw account data.
    pub total_len: u32,

    /// Number of raw account data bytes written so far.
    pub written_len: u32,

    /// Once true, buffer content cannot change except exact duplicate retries.
    pub finalized: bool,

    /// Keeps the header length 8-byte aligned before raw account data.
    pub _padding: [u8; 7],
}

impl StateBuffer {
    pub const DISCRIMINATOR: [u8; 8] = *b"v2sbuf00";
}
