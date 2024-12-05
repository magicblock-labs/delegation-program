use crate::state::utils::account::{AccountDiscriminator, Discriminator};
use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::pubkey::Pubkey;
use std::collections::BTreeSet;

#[derive(BorshSerialize, BorshDeserialize, Default, Debug)]
pub struct ProgramConfig {
    pub approved_validators: BTreeSet<Pubkey>,
}

impl Discriminator for ProgramConfig {
    fn discriminator() -> AccountDiscriminator {
        AccountDiscriminator::ProgramConfig
    }
}

impl ProgramConfig {
    pub fn serialized_len(&self) -> usize {
        4 + (self.approved_validators.len() * 32)
    }
}
