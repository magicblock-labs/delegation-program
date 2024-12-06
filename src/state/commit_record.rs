use bytemuck::{Pod, Zeroable};
use solana_program::pubkey::Pubkey;

use crate::state::utils::account::{AccountDiscriminator, AccountWithDiscriminator};
use crate::{impl_account_from_bytes, impl_to_bytes};

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct CommitRecordWithDiscriminator {
    pub discriminant: u64,
    pub value: CommitRecord,
}

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

    /// The account committed lamports
    pub lamports: u64,
}

impl AccountWithDiscriminator for CommitRecord {
    fn discriminator() -> AccountDiscriminator {
        AccountDiscriminator::CommitRecord
    }
}

impl_to_bytes!(CommitRecord);
impl_account_from_bytes!(CommitRecord);
