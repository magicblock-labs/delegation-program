use wheels::variable_offset_layout;

#[derive(Clone, Debug, PartialEq, Eq)]
#[variable_offset_layout(buffer_offset = 1)]
pub struct RaiseChallengeArgs {
    /// State commitment hash stored in the pending commitment being challenged.
    pub state_commitment_hash: [u8; 32],

    /// Salted hash binding the challenger state to this challenge.
    pub challenge_hash: [u8; 32],

    /// Lamports locked in the challenge account until reveal or resolution.
    pub stake_lamports: u64,
}
