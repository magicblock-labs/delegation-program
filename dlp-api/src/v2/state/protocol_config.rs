use wheels::fixed_offset_layout;

use crate::compat::Pubkey;

/// PDA: `["protocol-config"]`.
/// Created by `InitProtocolConfig`; not normally closed.
#[derive(Clone, Debug, PartialEq, Eq)]
#[fixed_offset_layout(buffer_offset = 0)]
pub struct ProtocolConfig {
    /// Account type marker.
    pub discriminator: [u8; 8],

    /// Signer allowed to update config and permissioned bootstrap state.
    pub authority: Pubkey,

    /// Emergency stop for new commitments and other non-exit activity.
    pub paused: bool,

    /// Multisig-controlled signer allowed to resolve disputes.
    pub resolver: Pubkey,

    /// Vault receiving protocol fees, penalties, or slashed funds.
    pub protocol_fee_vault: Pubkey,

    /// Minimum stake required for an operator to register and stay active.
    pub min_operator_bond: u64,

    /// Minimum stake required for a verifier to register and stay active.
    pub min_verifier_bond: u64,

    /// Minimum stake locked by a challenge to prevent cheap spam.
    pub min_challenger_stake: u64,

    /// Slots available for approval/challenge after commitment post.
    pub challenge_window_slots: u64,

    /// Slots the operator gets to open state after a challenge.
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

impl ProtocolConfig {
    pub const DISCRIMINATOR: [u8; 8] = *b"v2cfg000";
}
