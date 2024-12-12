use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::pubkey::Pubkey;

#[derive(Default, Debug, BorshSerialize, BorshDeserialize)]
pub struct DelegateArgs {
    pub unused_valid_until: i64, // Keeping for ABI backward compat (should be removed later)
    pub commit_frequency_ms: u32,
    pub seeds: Vec<Vec<u8>>,
    pub validator: Option<Pubkey>,
}
