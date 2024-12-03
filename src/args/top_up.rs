use borsh::{BorshDeserialize, BorshSerialize};

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct TopUpArgs {
    pub amount: u64,
    pub index: u8,
}
