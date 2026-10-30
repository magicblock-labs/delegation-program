use wheels::variable_offset_layout;

#[derive(Clone, Debug, PartialEq, Eq)]
#[variable_offset_layout(buffer_offset = 1)]
pub struct CloseStateBufferArgs {
    /// Commit sequence number that identifies the buffer and its possible commitment.
    pub commit_seq: u32,

    /// Canonical StateBuffer PDA bump. Set by the instruction builder.
    pub state_buffer_bump: u8,

    /// Canonical PendingCommitment PDA bump. Set by the instruction builder.
    pub pending_commitment_bump: u8,
}
