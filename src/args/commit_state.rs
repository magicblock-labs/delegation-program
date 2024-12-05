use borsh::{BorshDeserialize, BorshSerialize};

#[derive(Default, Debug, BorshSerialize, BorshDeserialize)]
pub struct CommitStateArgs {
    pub slot: u64,
    pub lamports: u64,
    pub allow_undelegation: bool,
    pub data: Vec<u8>,
}
