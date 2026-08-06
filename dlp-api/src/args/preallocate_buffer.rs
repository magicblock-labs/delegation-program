use borsh::{BorshDeserialize, BorshSerialize};

use crate::compat::borsh;

/// Which DLP-owned buffer (or the delegated account itself) a
/// `PreallocateBuffer` instruction should grow.
#[derive(
    Default, Debug, Clone, Copy, PartialEq, Eq, BorshSerialize, BorshDeserialize,
)]
pub enum PreallocateBufferKind {
    /// The `commit_state` PDA (seeds: `[COMMIT_STATE_TAG, delegated_account]`).
    #[default]
    CommitState,
    /// The `undelegate_buffer` PDA (seeds: `[UNDELEGATE_BUFFER_TAG, delegated_account]`).
    UndelegateBuffer,
    /// The delegated account itself. Only usable by the delegation's authority,
    /// since it mutates observable base-layer state ahead of a commit/finalize.
    DelegatedAccount,
}

#[derive(Default, Debug, BorshSerialize, BorshDeserialize)]
pub struct PreallocateBufferArgs {
    /// Which buffer to grow.
    pub kind: PreallocateBufferKind,
    /// The final size the buffer should reach. A single call grows the buffer
    /// towards this size by at most `MAX_PERMITTED_DATA_INCREASE` bytes, so
    /// callers send the same args repeatedly (once per top-level instruction)
    /// until the buffer reaches `target_size`.
    pub target_size: u32,
}
