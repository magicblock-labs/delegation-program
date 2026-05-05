use std::collections::BTreeSet;

use borsh_0_10::{BorshDeserialize, BorshSerialize};

use super::discriminator::{AccountDiscriminator, AccountWithDiscriminator};
use crate::{
    impl_to_bytes_with_discriminator_borsh,
    impl_try_from_bytes_with_discriminator_borsh,
    solana_program::pubkey::Pubkey,
};

#[derive(BorshSerialize, BorshDeserialize, Default, Debug)]
#[borsh(crate = "borsh_0_10")]
pub struct ProgramConfig {
    pub approved_validators: BTreeSet<Pubkey>,
}

impl AccountWithDiscriminator for ProgramConfig {
    fn discriminator() -> AccountDiscriminator {
        AccountDiscriminator::ProgramConfig
    }
}

impl ProgramConfig {
    pub fn size_with_discriminator(&self) -> usize {
        8 + 4 + 32 * self.approved_validators.len()
    }
}

impl_to_bytes_with_discriminator_borsh!(ProgramConfig);
impl_try_from_bytes_with_discriminator_borsh!(ProgramConfig);
