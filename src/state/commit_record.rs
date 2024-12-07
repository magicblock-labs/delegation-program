use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use solana_program::pubkey::Pubkey;

use crate::{
    impl_to_bytes_with_discriminator_zero_copy, impl_try_from_bytes_with_discriminator_zero_copy,
};

use super::discriminator::AccountDiscriminator;

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

impl CommitRecord {
    pub fn discriminator() -> [u8; 8] {
        AccountDiscriminator::CommitRecord.to_bytes()
    }
    pub fn size_with_discriminator() -> usize {
        8 + size_of::<CommitRecord>()
    }
}

impl_to_bytes_with_discriminator_zero_copy!(CommitRecord);
impl_try_from_bytes_with_discriminator_zero_copy!(CommitRecord);
