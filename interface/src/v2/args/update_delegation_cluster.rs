use wheels::variable_offset_layout;

use crate::v2::ChallengeConfig;

/// Mutable challenge policy for an existing delegation cluster.
#[derive(Clone, Debug, PartialEq, Eq)]
#[variable_offset_layout(buffer_offset = 1)]
pub struct UpdateDelegationClusterArgs {
    /// Replacement challenge policy. Must be `Some`; cluster mode is immutable.
    pub challenge_config: Option<ChallengeConfig>,
}
