use std::mem::size_of;

use crate::consts::DELEGATION_RECORD_DISCRIMINANT;
use crate::{
    impl_to_bytes_without_discriminant_zero_copy, impl_try_from_bytes_with_discriminant_zero_copy,
};
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

    /// The lamports at the time of delegation or from the last state finalization, stored as lamports can be received even if the account is delegated
    pub lamports: u64,

    /// The state update frequency in milliseconds
    pub commit_frequency_ms: u64,
}

impl DelegationRecord {
    pub fn discriminant() -> &'static [u8; 8] {
        return DELEGATION_RECORD_DISCRIMINANT;
    }
    pub fn size_with_discriminant() -> usize {
        8 + size_of::<DelegationRecord>()
    }
}

impl_to_bytes_without_discriminant_zero_copy!(DelegationRecord);
impl_try_from_bytes_with_discriminant_zero_copy!(DelegationRecord);
