use wheels::variable_offset_layout;

use crate::compat::Pubkey;

#[derive(Clone, Debug, PartialEq, Eq)]
#[variable_offset_layout(buffer_offset = 1)]
pub struct ChallengerRevealArgs {
    /// Challenger-revealed account lamports.
    pub lamports: u64,

    /// Challenger-revealed account owner.
    pub owner: Pubkey,

    /// Hash of the full challenger-uploaded account data.
    pub data_hash: [u8; 32],

    /// Salt used to open the challenge hash.
    pub salt: [u8; 32],
}
