use wheels::variable_offset_layout;

#[derive(Clone, Debug, PartialEq, Eq)]
#[variable_offset_layout(buffer_offset = 1)]
pub struct UndelegateArgs {
    /// Bump for the temporary DLP-owned undelegation buffer PDA.
    /// Set by the instruction builder.
    pub undelegate_buffer_bump: u8,
}
