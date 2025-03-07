use borsh::{BorshDeserialize, BorshSerialize};

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct TopUpEphemeralBalanceArgs {
    /// The amount to add to the ephemeral balance.
    pub amount: u64,
    /// The index of the ephemeral balance account to top up which allows
    /// on payer to have multiple ephemeral balance accounts.
    pub index: u8,
}
