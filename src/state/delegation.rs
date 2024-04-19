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

    /// The origin authority
    pub origin: Pubkey,

    /// The delegation validity
    pub valid_until: i64,
}

impl Discriminator for Delegation {
    fn discriminator() -> AccountDiscriminator {
        AccountDiscriminator::Authority
    }
}

impl_to_bytes!(Delegation);
impl_account_from_bytes!(Delegation);
