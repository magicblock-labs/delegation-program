use crate::utils::utils_account::{AccountDiscriminator, Discriminator};
use crate::{impl_account_from_bytes, impl_to_bytes};
use bytemuck::{Pod, Zeroable};
use solana_program::pubkey::Pubkey;

/// The Delegation Record stores information such as the authority, the owner and the commit frequency.
/// This is used by the ephemeral validator to update the state of the delegated account.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct DelegationRecord {
    /// The delegated authority
    pub authority: Pubkey,

    /// The original owner of the account
    pub owner: Pubkey,

    /// The slot at which the delegation was created
    pub delegation_slot: u64,

    /// The state update frequency in milliseconds
    pub commit_frequency_ms: u64,
}

impl Discriminator for DelegationRecord {
    fn discriminator() -> AccountDiscriminator {
        AccountDiscriminator::DelegationRecord
    }
}

impl_to_bytes!(DelegationRecord);
impl_account_from_bytes!(DelegationRecord);
