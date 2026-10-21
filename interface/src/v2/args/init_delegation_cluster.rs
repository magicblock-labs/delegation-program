use wheels::variable_offset_layout;

use crate::{compat::Pubkey, v2::ChallengeConfig};

#[derive(Clone, Debug, PartialEq, Eq)]
#[variable_offset_layout(buffer_offset = 1)]
pub struct InitDelegationClusterArgs {
    /// Validator initially authorized to act for the cluster.
    pub current_validator: Pubkey,

    /// Signer allowed to rotate the cluster's validator.
    pub validator_rotation_authority: Pubkey,

    /// Challenge policy, or `None` for direct finalization.
    pub challenge_config: Option<ChallengeConfig>,
}
