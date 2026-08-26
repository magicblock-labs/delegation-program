use wheels::variable_offset_layout;

use crate::compat::Pubkey;

#[derive(Clone, Debug, PartialEq, Eq)]
#[variable_offset_layout(buffer_offset = 0)]
pub struct PostCommitmentArgs {
    pub commit_id: u64,

    pub lamports: u64,

    pub owner: Pubkey,

    /// Hash of replay/data-availability pointer bytes.
    pub da_pointer_hash: [u8; 32],

    /// ER slot that produced this commitment, when available.
    pub er_slot: Option<u64>,
}
