use wheels::variable_offset_layout;

/// One owner-program seed used to derive the delegated account.
#[derive(Clone, Debug, PartialEq, Eq)]
#[variable_offset_layout(buffer_offset = unknown)]
pub struct DelegationSeed {
    #[flexible = 1]
    pub value: Vec<u8>,
}

impl<'a> DelegationSeedView<'a> {
    /// Returns the seed without its one-byte variable-layout length prefix.
    pub fn as_slice(&self) -> &'a [u8] {
        &self.bytes()[1..]
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[variable_offset_layout(buffer_offset = 1)]
pub struct DelegateArgs {
    /// Bump for the owner-program delegate buffer PDA.
    /// Set by the instruction builder.
    pub delegate_buffer_bump: u8,

    /// Bump for the DLP-owned DelegationState PDA.
    /// Set by the instruction builder.
    pub delegation_state_bump: u8,

    /// Bump used with `seeds` to derive an off-curve delegated account.
    /// Must be zero for an on-curve delegated account.
    pub delegated_account_bump: u8,

    /// Owner-program seeds used to derive an off-curve delegated account.
    #[flexible = 1]
    #[element_size = variable]
    pub seeds: Vec<DelegationSeed>,
}

impl DelegateArgs {
    pub fn new(delegated_account_bump: u8, seeds: Vec<DelegationSeed>) -> Self {
        Self {
            delegate_buffer_bump: 0,
            delegation_state_bump: 0,
            delegated_account_bump,
            seeds,
        }
    }
}
