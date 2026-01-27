use std::mem::size_of;

use borsh::{BorshDeserialize, BorshSerialize};
use bytemuck::{Pod, Zeroable};

use crate::args::{ArgsWithBuffer, Boolean};

/// bumps of the PDA accounts to be validated by ix
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable, Default)]
pub struct CommitBumps {
    pub delegation_record: u8,
    pub delegation_metadata: u8,
    pub validator_fees_vault: u8,
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct CommitFinalizeArgs {
    /// the commit_id ensures correct ordering of commits
    pub commit_id: u64,

    /// the lamports that the delegated account holds in the ephemeral validator
    pub lamports: u64,

    /// whether the account can be undelegated after the commit completes
    pub allow_undelegation: Boolean,

    /// whether the data (in the ixdata or in the data account) is diff or full state.
    pub data_is_diff: Boolean,

    /// bumps of the PDA accounts to be validated by the ix
    pub bumps: CommitBumps,

    pub reserved_padding: [u8; 3],
}

pub type CommitFinalizeArgsWithBuffer<'a> =
    ArgsWithBuffer<'a, CommitFinalizeArgs>;

#[derive(Default, Debug, BorshSerialize, BorshDeserialize)]
pub struct CommitStateArgs {
    /// "Nonce" of an account. Updates are submitted historically and nonce incremented by 1
    /// Deprecated: The ephemeral slot at which the account data is committed
    pub nonce: u64,
    /// The lamports that the account holds in the ephemeral validator
    pub lamports: u64,
    /// Whether the account can be undelegated after the commit completes
    pub allow_undelegation: bool,
    /// The account data
    pub data: Vec<u8>,
}

#[derive(Default, Debug, BorshSerialize, BorshDeserialize)]
pub struct CommitStateFromBufferArgs {
    /// "Nonce" of an account. Updates are submitted historically and nonce incremented by 1
    /// Deprecated: The ephemeral slot at which the account data is committed
    pub nonce: u64,
    /// The lamports that the account holds in the ephemeral validator
    pub lamports: u64,
    /// Whether the account can be undelegated after the commit completes
    pub allow_undelegation: bool,
}

#[derive(Default, Debug, BorshSerialize)]
pub struct CommitDiffArgs {
    /// The account diff
    /// SAFETY: this must be the FIRST field in the struct because the serialized format
    /// is manually split: the diff (with Borsh Vec prefix) followed by the fixed-size
    /// fields. The processor uses `data.split_at(data.len() - SIZE_COMMIT_DIFF_ARGS_WITHOUT_DIFF)`
    /// to separate them during deserialization.
    pub diff: Vec<u8>,

    /// "Nonce" of an account. Updates are submitted historically and nonce incremented by 1
    /// Deprecated: The ephemeral slot at which the account data is committed
    pub nonce: u64,

    /// The lamports that the account holds in the ephemeral validator
    pub lamports: u64,

    /// Whether the account can be undelegated after the commit completes
    pub allow_undelegation: bool,
}

#[derive(Default, Debug, BorshDeserialize)]
pub struct CommitDiffArgsWithoutDiff {
    /// "Nonce" of an account. Updates are submitted historically and nonce incremented by 1
    /// Deprecated: The ephemeral slot at which the account data is committed
    pub nonce: u64,
    /// The lamports that the account holds in the ephemeral validator
    pub lamports: u64,
    /// Whether the account can be undelegated after the commit completes
    pub allow_undelegation: bool,
}

pub const SIZE_COMMIT_DIFF_ARGS_WITHOUT_DIFF: usize =
    size_of::<u64>() + size_of::<u64>() + size_of::<bool>();
