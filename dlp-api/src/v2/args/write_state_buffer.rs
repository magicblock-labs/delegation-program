use wheels::variable_offset_layout;

#[derive(Clone, Debug, PartialEq, Eq)]
#[variable_offset_layout(buffer_offset = 1)]
pub struct WriteStateBufferArgs {
    pub commit_id: u64,

    pub total_len: u32,

    /// Must equal bytes already written, unless retrying an exact old chunk.
    pub offset: u32,

    #[flexible = 4]
    pub chunk: Vec<u8>,
}
