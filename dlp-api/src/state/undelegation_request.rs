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
    /// The delegated account this request targets.
    pub delegated_account: Pubkey,

    /// The original owner program recorded for the delegated account.
    pub owner_program: Pubkey,

    /// The account that paid rent for this request PDA.
    pub rent_payer: Pubkey,

    /// The slot at which the request was created.
    pub created_slot: u64,
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
