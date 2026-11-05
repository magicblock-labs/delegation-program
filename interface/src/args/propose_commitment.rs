use wheels::variable_offset_layout;

use crate::compat::Pubkey;

#[derive(Clone, Debug, PartialEq, Eq)]
#[variable_offset_layout(buffer_offset = 1)]
pub struct ProposeCommitmentArgs {
    pub commit_seq: u32,

    pub lamports: u64,

    pub owner: Pubkey,

    /// Whether finalization should make the account undelegatable.
    pub allow_undelegation: bool,

    /// Hash of replay/data-availability pointer bytes.
    pub da_pointer_hash: [u8; 32],

    /// ER slot that produced this commitment, when available.
    pub er_slot: Option<u64>,
}
