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
    /// Pre-allocating delegated account itself
    DelegatedAccount,
}

#[derive(Default, Debug, BorshSerialize, BorshDeserialize)]
pub struct PreallocateBufferArgs {
    /// Which buffer to grow.
    pub kind: PreallocateBufferKind,
    /// The final size the buffer should reach
    pub target_size: u32,
}
