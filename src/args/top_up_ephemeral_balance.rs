use borsh::{BorshDeserialize, BorshSerialize};

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct TopUpEphemeralBalanceArgs {
    pub amount: u64,
    pub index: u8,
}
