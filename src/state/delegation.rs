use bytemuck::{Pod, Zeroable};
use solana_program::pubkey::Pubkey;

use crate::{
    impl_account_from_bytes, impl_to_bytes,
    utils::{AccountDiscriminator, Discriminator},
};

/// The Delegation Record
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct DelegationRecord {
    /// The delegated authority
    pub authority: Pubkey,

    /// The original owner of the account
    pub owner: Pubkey,

    /// The delegation validity
    pub valid_until: i64,

    /// The state update frequency in milliseconds
    pub commit_frequency_ms: u64,
}

impl Discriminator for DelegationRecord {
    fn discriminator() -> AccountDiscriminator {
        AccountDiscriminator::Delegation
    }
}

impl_to_bytes!(DelegationRecord);
impl_account_from_bytes!(DelegationRecord);
