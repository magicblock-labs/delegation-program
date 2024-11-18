use crate::utils::utils_account::{AccountDiscriminator, Discriminator};
use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::pubkey::Pubkey;
use std::collections::BTreeSet;

#[derive(BorshSerialize, BorshDeserialize, Default, Debug)]
pub struct WhitelistForProgram {
    pub approved_validators: BTreeSet<Pubkey>,
}

impl Discriminator for WhitelistForProgram {
    fn discriminator() -> AccountDiscriminator {
        AccountDiscriminator::WhitelistForProgram
    }
}

impl WhitelistForProgram {
    pub fn serialized_len(&self) -> usize {
        4 + (self.approved_validators.len() * 32)
    }
}
