use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::pubkey::Pubkey;

use crate::{impl_to_bytes_with_discriminator_borsh, impl_try_from_bytes_with_discriminator_borsh};

use super::discriminator::{AccountDiscriminator, AccountWithDiscriminator};

#[derive(BorshSerialize, BorshDeserialize, Default, Debug)]
pub struct FeesVault {
    pub fees_receiver: Pubkey,
}

impl AccountWithDiscriminator for FeesVault {
    fn discriminator() -> AccountDiscriminator {
        AccountDiscriminator::FeesVault
    }
}

impl FeesVault {
    pub fn size_with_discriminator(&self) -> usize {
        8 + 32
    }
}

impl_to_bytes_with_discriminator_borsh!(FeesVault);
impl_try_from_bytes_with_discriminator_borsh!(FeesVault);
