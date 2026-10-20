use wheels::fixed_offset_layout;

use crate::compat::Pubkey;

/// Challenge-only policy for a delegation cluster.
#[derive(Clone, Debug, PartialEq, Eq)]
#[fixed_offset_layout(buffer_offset = unknown)]
pub struct ChallengeConfig {
    /// Multisig-controlled signer allowed to resolve disputes.
    pub resolver: Pubkey,

    /// Minimum stake required for an operator to register and stay active.
    pub min_operator_bond: u64,

    /// Minimum stake required for a verifier to register and stay active.
    pub min_verifier_bond: u64,

    /// Minimum stake locked by a challenge to prevent cheap spam.
    pub min_challenger_stake: u64,

    /// Slots available for approval/challenge after commitment post.
    pub challenge_window_slots: u64,

    /// CHECKPOINT: Reserved for a possible future operator-response phase; the
    /// current flow writes operator state before posting the commitment.
    pub operator_response_timeout_slots: u64,

    /// Slots the challenger gets to reveal after operator response or timeout.
    pub challenger_reveal_timeout_slots: u64,

    /// Delay before a winning challenger can claim payout.
    pub payout_timelock_slots: u64,

    /// Maximum number of verifiers selected for one commitment.
    pub verifiers_per_commitment: u16,

    /// Approvals required for happy-path finalization.
    pub approval_threshold: u16,

    /// Maximum under-approval extensions before the commitment expires.
    pub max_window_extensions: u16,

    /// Penalty for a valid reveal that matches the operator state.
    pub match_penalty_bps: u16,
}

impl ChallengeConfig {
    /// Maximum value for basis-points config fields.
    pub const MAX_MATCH_PENALTY_BPS: u16 = 10_000;
}

/// PDA: `["delegation-cluster", cluster_id]`.
/// Created by `InitDelegationCluster`; not normally closed.
#[derive(Clone, Debug, PartialEq, Eq)]
#[fixed_offset_layout(buffer_offset = 0)]
pub struct DelegationCluster {
    /// Account type marker.
    pub discriminator: [u8; 8],

    /// Canonical PDA bump for this account.
    pub bump: u8,

    /// Stable identifier used to derive this cluster account.
    pub cluster_id: Pubkey,

    /// Signer allowed to update cluster policy and bootstrap state.
    pub config_authority: Pubkey,

    /// Signer allowed to replace the current validator.
    pub validator_rotation_authority: Pubkey,

    /// Validator currently authorized to act for this cluster.
    pub current_validator: Pubkey,

    /// Emergency stop for new commitments and other non-exit activity.
    pub paused: bool,

    /// Vault receiving protocol fees, penalties, or slashed funds.
    pub protocol_fee_vault: Pubkey,

    /// `None` selects direct finalization; `Some` enables fraud proofs.
    #[flexible]
    pub challenge_config: Option<ChallengeConfig>,
}

impl DelegationCluster {
    pub const DISCRIMINATOR: [u8; 8] = *b"v2clstr0";
}
