use bytemuck::{Pod, Zeroable};
use solana_program::pubkey::Pubkey;

use crate::utils_account::{AccountDiscriminator, Discriminator};
use crate::{impl_account_from_bytes, impl_to_bytes};

/// The Commit State Record
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct CommitRecord {
    /// The identity committing the state
    pub identity: Pubkey,

    /// The account for which the state is committed
    pub account: Pubkey,

    /// The external slot of the commit. This is used to enforce sequential commits
    pub slot: u64,
}

impl Discriminator for CommitRecord {
    fn discriminator() -> AccountDiscriminator {
        AccountDiscriminator::CommitRecord
    }
}

impl_to_bytes!(CommitRecord);
impl_account_from_bytes!(CommitRecord);
