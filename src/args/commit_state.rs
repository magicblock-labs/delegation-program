use borsh::{BorshDeserialize, BorshSerialize};

#[derive(Default, Debug, BorshSerialize, BorshDeserialize)]
pub struct CommitStateArgs {
    /// "Nonce" of an account. Updates are submitted historically and nonce incremented by 1
    /// Deprecated: The ephemeral slot at which the account data is committed
    pub nonce: u64,
    /// The lamports that the account holds in the ephemeral validator
    pub lamports: u64,
    /// Whether the account can be undelegated after the commit completes
    pub allow_undelegation: bool,
    /// The account data
    pub data: Vec<u8>,
}

#[derive(Default, Debug, BorshSerialize, BorshDeserialize)]
pub struct CommitStateFromBufferArgs {
    /// "Nonce" of an account. Updates are submitted historically and nonce incremented by 1
    /// Deprecated: The ephemeral slot at which the account data is committed
    pub nonce: u64,
    /// The lamports that the account holds in the ephemeral validator
    pub lamports: u64,
    /// Whether the account can be undelegated after the commit completes
    pub allow_undelegation: bool,
}
