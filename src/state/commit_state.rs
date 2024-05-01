use bytemuck::{Pod, Zeroable};
use shank::ShankAccount;
use solana_program::pubkey::Pubkey;

use crate::{
    impl_account_from_bytes, impl_to_bytes,
    utils::{AccountDiscriminator, Discriminator},
};

/// The Commit State Record
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, ShankAccount, Zeroable)]
pub struct CommitState {
    /// The identity committing the state
    pub identity: Pubkey,

    /// The account for which the state is committed
    pub account: Pubkey,

    /// The timestamp of the commit. NB: This is not used a reliable source of time.
    pub timestamp: i64,
}

impl Discriminator for CommitState {
    fn discriminator() -> AccountDiscriminator {
        AccountDiscriminator::CommitState
    }
}

impl_to_bytes!(CommitState);
impl_account_from_bytes!(CommitState);
