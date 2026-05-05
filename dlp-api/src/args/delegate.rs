use borsh_0_10::{BorshDeserialize, BorshSerialize};

use crate::solana_program::pubkey::Pubkey;

#[derive(Default, Debug, BorshSerialize, BorshDeserialize)]
#[borsh(crate = "borsh_0_10")]
pub struct DelegateArgs {
    /// The frequency at which the validator should commit the account data
    /// if no commit is triggered by the owning program
    pub commit_frequency_ms: u32,
    /// The seeds used to derive the PDA of the delegated account
    pub seeds: Vec<Vec<u8>>,
    /// The validator authority that is added to the delegation record
    pub validator: Option<Pubkey>,
}
