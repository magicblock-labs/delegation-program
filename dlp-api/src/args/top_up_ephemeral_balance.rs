use borsh_0_10::{BorshDeserialize, BorshSerialize};

#[derive(Debug, BorshSerialize, BorshDeserialize)]
#[borsh(crate = "borsh_0_10")]
pub struct TopUpEphemeralBalanceArgs {
    /// The amount to add to the ephemeral balance.
    pub amount: u64,
    /// The index of the ephemeral balance account to top up which allows
    /// one payer to have multiple ephemeral balance accounts.
    pub index: u8,
}
