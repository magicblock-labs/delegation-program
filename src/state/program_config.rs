use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::pubkey::Pubkey;
use std::collections::BTreeSet;

use crate::{impl_to_bytes_with_discriminator_borsh, impl_try_from_bytes_with_discriminator_borsh};

use super::discriminator::{AccountDiscriminator, AccountWithDiscriminator};

#[derive(BorshSerialize, BorshDeserialize, Default, Debug)]
pub struct ProgramConfig {
    pub approved_validators: BTreeSet<Pubkey>,
    pub fees_receiver: Pubkey,
}

impl AccountWithDiscriminator for ProgramConfig {
    fn discriminator() -> AccountDiscriminator {
        AccountDiscriminator::ProgramConfig
    }
}

impl ProgramConfig {
    pub fn size_with_discriminator(&self) -> usize {
        8 + 4 + 32 * self.approved_validators.len() + 32
    }
}

impl_to_bytes_with_discriminator_borsh!(ProgramConfig);
impl_try_from_bytes_with_discriminator_borsh!(ProgramConfig);
