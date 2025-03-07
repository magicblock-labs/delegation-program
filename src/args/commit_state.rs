use borsh::{BorshDeserialize, BorshSerialize};

#[derive(Default, Debug, BorshSerialize, BorshDeserialize)]
pub struct CommitStateArgs {
    /// The ephemeral slot at which the account data is committed
    pub slot: u64,
    /// The lamports that the account holds in the ephemeral validator
    pub lamports: u64,
    /// Whether the account can be undelegated after the commit completes
    pub allow_undelegation: bool,
    /// The account data
    pub data: Vec<u8>,
}

#[derive(Default, Debug, BorshSerialize, BorshDeserialize)]
pub struct CommitStateFromBufferArgs {
    /// The ephemeral slot at whih the account data is committed
    pub slot: u64,
    /// The lamports that the account holds in the ephemeral validator
    pub lamports: u64,
    /// Whether the account can be undelegated after the commit completes
    pub allow_undelegation: bool,
}
