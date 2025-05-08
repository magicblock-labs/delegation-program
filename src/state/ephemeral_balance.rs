use crate::state::discriminator::{AccountDiscriminator, AccountWithDiscriminator};
use crate::{impl_to_bytes_with_discriminator_borsh, impl_try_from_bytes_with_discriminator_borsh};
use borsh::{BorshDeserialize, BorshSerialize};

#[derive(BorshSerialize, BorshDeserialize, Debug, PartialEq)]
pub struct EphemeralBalance;

impl AccountWithDiscriminator for EphemeralBalance {
    fn discriminator() -> AccountDiscriminator {
        AccountDiscriminator::EphemeralBalance
    }
}

impl_to_bytes_with_discriminator_borsh!(EphemeralBalance);
impl_try_from_bytes_with_discriminator_borsh!(EphemeralBalance);
