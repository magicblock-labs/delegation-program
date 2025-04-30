use borsh::{BorshDeserialize, BorshSerialize};

#[derive(BorshSerialize, BorshDeserialize)]
pub enum Context {
    Commit,
    Undelegate,
    Standalone,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct CallHandlerArgs {
    pub escrow_index: u8,
    pub data: Vec<u8>,
    pub context: Context
}
