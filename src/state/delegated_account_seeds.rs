use crate::utils::{AccountDiscriminator, Discriminator};
use borsh::{BorshDeserialize, BorshSerialize};

/// The Delegated Account Seeds
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct DelegateAccountSeeds {
    pub seeds: Vec<Vec<u8>>,
}

impl Discriminator for DelegateAccountSeeds {
    fn discriminator() -> AccountDiscriminator {
        AccountDiscriminator::DelegationRecord
    }
}
