use bytemuck::{Pod, Zeroable};
use shank::ShankAccount;
use solana_program::pubkey::Pubkey;

use crate::{
    impl_account_from_bytes, impl_to_bytes,
    utils::{AccountDiscriminator, Discriminator},
};

///
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, ShankAccount, Zeroable)]
pub struct Delegation {
    ///
    pub authority: Pubkey,

    ///
    pub origin: Pubkey,
}

impl Discriminator for Delegation {
    fn discriminator() -> AccountDiscriminator {
        AccountDiscriminator::Authority
    }
}

impl_to_bytes!(Delegation);
impl_account_from_bytes!(Delegation);
