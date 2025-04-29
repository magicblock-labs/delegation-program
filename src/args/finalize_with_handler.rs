use borsh::{BorshDeserialize, BorshSerialize};

#[derive(BorshSerialize, BorshDeserialize)]
pub struct FinalizeWithHookArgs {
    pub escrow_index: u8,
    pub data: Vec<u8>,
}
