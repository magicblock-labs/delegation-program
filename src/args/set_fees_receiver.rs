use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::pubkey::Pubkey;

#[derive(BorshSerialize, BorshDeserialize)]
pub struct SetFeesReceiverArgs {
    pub fees_receiver: Pubkey,
}
