use std::mem::size_of;

use bytemuck::{Pod, Zeroable};

use super::discriminator::{AccountDiscriminator, AccountWithDiscriminator};
use crate::{
    compat::Pubkey, impl_to_bytes_with_discriminator_zero_copy,
    impl_try_from_bytes_with_discriminator_zero_copy,
};

/// Request for the validator to undelegate one delegated account.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct UndelegationRequest {
    /// Delegated account this request is for.
    ///
    /// DLP validates the request by PDA seeds, so this is stored only to let
    /// validators identify the matching delegated account when scanning
    /// request accounts.
    pub delegated_account: Pubkey,
    /// The first slot at which timeout rollback is allowed.
    pub expires_at_slot: u64,
}

impl AccountWithDiscriminator for UndelegationRequest {
    fn discriminator() -> AccountDiscriminator {
        AccountDiscriminator::UndelegationRequest
    }
}

impl UndelegationRequest {
    pub fn size_with_discriminator() -> usize {
        8 + size_of::<UndelegationRequest>()
    }
}

impl_to_bytes_with_discriminator_zero_copy!(UndelegationRequest);
impl_try_from_bytes_with_discriminator_zero_copy!(UndelegationRequest);
