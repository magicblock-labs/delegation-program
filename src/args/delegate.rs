use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::pubkey::Pubkey;

#[derive(Default, Debug, BorshSerialize, BorshDeserialize)]
pub struct DelegateArgs {
    /// The frequency at which the validator should commit the account data
    /// if no commit is triggered by the owning program
    /// TODO: @@@ is this obsolete?
    pub commit_frequency_ms: u32,
    /// The seeds used to derive the PDA of the delegated account
    pub seeds: Vec<Vec<u8>>,
    /// The validator authority that is added to the delegation record
    pub validator: Option<Pubkey>,
}
