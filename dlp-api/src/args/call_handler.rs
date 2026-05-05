use borsh_0_10::{BorshDeserialize, BorshSerialize};

#[derive(BorshSerialize, BorshDeserialize)]
#[borsh(crate = "borsh_0_10")]
pub struct CallHandlerArgs {
    pub escrow_index: u8,
    /// This is raw instruction data, it could include discriminator + args
    /// or can be in any other custom format
    pub data: Vec<u8>,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct CallHandlerArgsX {
    pub escrow_index: u8,
    /// This is raw instruction data, it could include discriminator + args
    /// or can be in any other custom format
    pub data: Vec<u8>,
}
