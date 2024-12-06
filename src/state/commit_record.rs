use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use solana_program::pubkey::Pubkey;

use crate::{consts::COMMIT_RECORD_DISCRIMINANT, impl_try_from_bytes_with_discriminant_zero_copy};

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
    pub fn discriminant() -> &'static [u8; 8] {
        return COMMIT_RECORD_DISCRIMINANT;
    }
    pub fn size_with_discriminant() -> usize {
        8 + size_of::<CommitRecord>()
    }
}

impl_try_from_bytes_with_discriminant_zero_copy!(CommitRecord);
