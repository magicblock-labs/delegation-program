use borsh::{BorshDeserialize, BorshSerialize};
use light_sdk::instruction::{
    account_meta::CompressedAccountMeta, PackedAddressTreeInfo, ValidityProof,
};
use solana_program::pubkey::Pubkey;

#[derive(Default, Debug, BorshSerialize, BorshDeserialize)]
pub struct DelegateCompressedArgs {
    /// The frequency at which the validator should commit the account data
    /// if no commit is triggered by the owning program
    pub commit_frequency_ms: u32,
    /// The seeds used to derive the PDA of the delegated account
    pub seeds: Vec<Vec<u8>>,
    /// The validator authority that is added to the delegation record
    pub validator: Option<Pubkey>,
    /// The proof of the account data
    pub proof: ValidityProof,
    /// The address tree info
    pub address_tree_info: PackedAddressTreeInfo,
    /// The output state tree index
    pub output_state_tree_index: u8,
    /// The account meta
    pub account_meta: CompressedAccountMeta,
    /// The account data
    pub account_data: Vec<u8>,
}
