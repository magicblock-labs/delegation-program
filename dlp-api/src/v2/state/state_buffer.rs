use wheels::fixed_offset_layout;

use crate::{compat::Pubkey, error::DlpError};

/// PDA: `["state-buffer", account, commit_id, authority]`.
/// Created by `WriteStateBuffer`.
/// Closed by `CloseTerminalAccounts` after finalize, cancel, or expiry.
#[derive(Clone, Debug, PartialEq, Eq)]
#[fixed_offset_layout(buffer_offset = 0)]
pub struct StateBuffer {
    /// Account type marker.
    pub discriminator: [u8; 8],

    /// Writer that owns this opened buffer.
    ///
    /// This is the operator identity for an operator commitment buffer, or the
    /// challenger identity for a challenger dispute buffer.
    pub authority: Pubkey,

    /// Delegated account whose payload is stored after this header.
    pub account_pubkey: Pubkey,

    /// Flow-specific nonce that identifies this opened buffer.
    pub commit_id: u64,

    /// Hash of the finalized payload. Zero until finalized.
    pub data_hash: [u8; 32],

    /// Expected final byte length of the payload.
    pub total_len: u32,

    /// Once true, buffer content cannot change except exact duplicate retries.
    pub finalized: bool,

    /// Active payload bytes for `PostCommitment` or `RaiseChallenge`.
    ///
    /// This is not a serialized Solana account. Depending on the flow, it can
    /// be the delegated account's complete `Account::data` bytes or an encoded
    /// diff of those bytes. `payload.len()` is the written prefix, while
    /// `payload.capacity()` is the allocated account-backed span and can be
    /// shorter than `total_len` until later writes grow the buffer account.
    #[extendable = 4]
    pub payload: Vec<u8>,
}

impl StateBuffer {
    pub const DISCRIMINATOR: [u8; 8] = *b"v2sbuf00";

    /// Maximum account data bytes a StateBuffer PDA may allocate.
    pub const MAX_ACCOUNT_DATA_LEN: usize = 10 * 1024 * 1024;

    /// Maximum account data bytes a StateBuffer PDA may grow in one write.
    pub const MAX_ACCOUNT_DATA_GROWTH_PER_WRITE: usize = 10_240;

    /// Offset of the extendable payload length header.
    pub const PAYLOAD_LEN_HEADER_OFFSET: usize = Self::MIN_DATA_LEN;

    /// Byte length of the extendable payload length header.
    pub const PAYLOAD_LEN_HEADER_LEN: usize = 4;

    /// Offset where payload bytes begin in account data.
    pub const PAYLOAD_BYTES_OFFSET: usize =
        Self::PAYLOAD_LEN_HEADER_OFFSET + Self::PAYLOAD_LEN_HEADER_LEN;

    /// Maximum payload bytes allocated when a StateBuffer PDA is created.
    pub const MAX_INITIAL_PAYLOAD_LEN: usize =
        Self::MAX_ACCOUNT_DATA_GROWTH_PER_WRITE - Self::PAYLOAD_BYTES_OFFSET;

    /// Maximum payload bytes accepted across all writes.
    pub const MAX_TOTAL_PAYLOAD_LEN: u32 =
        (Self::MAX_ACCOUNT_DATA_LEN - Self::PAYLOAD_BYTES_OFFSET) as u32;

    /// Returns the serialized data length needed for a payload capacity.
    pub fn data_len_from_payload_capacity(
        payload_capacity: usize,
    ) -> Result<usize, DlpError> {
        Self::PAYLOAD_BYTES_OFFSET
            .checked_add(payload_capacity)
            .ok_or(DlpError::Overflow)
    }
}
