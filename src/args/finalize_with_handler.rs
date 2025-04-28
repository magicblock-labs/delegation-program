use borsh::{BorshDeserialize, BorshSerialize};

#[derive(BorshSerialize, BorshDeserialize)]
pub struct FinalizeWithDataArgs {
    // TODO: this shall be passed, since excrow there could be multiple escrows
    // TODO: do we even need escrow for post commit action?
    pub escrow_index: u8,
    // TODO: could be arbitrary data
    pub data: Vec<u8>,
    // TODO: do we need this?. Can be used
    // destination_program: Pubkey,
}
