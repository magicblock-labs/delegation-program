use bytemuck::{Pod, Zeroable};
use shank::ShankAccount;
use solana_program::pubkey::Pubkey;

use crate::{
    impl_account_from_bytes, impl_to_bytes,
    utils::{AccountDiscriminator, Discriminator},
};

/// The Delegation Record
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, ShankAccount, Zeroable)]
pub struct Delegation {
    /// The delegated authority
    pub authority: Pubkey,

    /// The original owner of the account
    pub origin: Pubkey,

    /// The delegation validity
    pub valid_until: i64,

    /// The number of committed states for the delegated account
    pub commits: u64,
}

impl Discriminator for Delegation {
    fn discriminator() -> AccountDiscriminator {
        AccountDiscriminator::Delegation
    }
}

impl_to_bytes!(Delegation);
impl_account_from_bytes!(Delegation);