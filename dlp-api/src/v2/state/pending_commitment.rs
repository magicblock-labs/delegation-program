use wheels::fixed_offset_layout;

use crate::compat::Pubkey;

pub const PENDING_COMMITMENT_STATUS_ACTIVE: u8 = 1;
pub const PENDING_COMMITMENT_STATUS_AWAITING_OPERATOR_RESPONSE: u8 = 2;
pub const PENDING_COMMITMENT_STATUS_AWAITING_CHALLENGER_REVEAL: u8 = 3;
pub const PENDING_COMMITMENT_STATUS_AWAITING_DISPUTE_RESOLUTION: u8 = 4;
pub const PENDING_COMMITMENT_STATUS_RESOLVED_OPERATOR: u8 = 5;
pub const PENDING_COMMITMENT_STATUS_RESOLVED_CHALLENGER: u8 = 6;
pub const PENDING_COMMITMENT_STATUS_FINALIZED: u8 = 7;
pub const PENDING_COMMITMENT_STATUS_EXPIRED: u8 = 8;
pub const PENDING_COMMITMENT_STATUS_CANCELLED: u8 = 9;

pub const RESOLVED_STATE_SOURCE_OPERATOR_COMMITMENT: u8 = 1;
pub const RESOLVED_STATE_SOURCE_CHALLENGER_REVEAL: u8 = 2;

/// PDA: `["pending-commitment", account, commit_id]`.
/// Created by `PostCommitment`.
/// Closed by `CloseTerminalAccounts` after finalize, cancel, or expiry.
///
/// One account per delegated account and commit id.
#[derive(Clone, Debug, PartialEq, Eq)]
#[fixed_offset_layout(buffer_offset = 0)]
pub struct PendingCommitment {
    /// Account type marker.
    pub discriminator: [u8; 8],

    /// Current state-machine state for this commitment.
    pub status: u8,

    /// Operator identity that posted the commitment.
    pub operator_identity: Pubkey,

    /// OperatorBond checked when the commitment was posted.
    pub operator_bond: Pubkey,

    /// Delegated account whose base-layer state will be finalized.
    pub account_pubkey: Pubkey,

    /// Operator-chosen nonce for this account commitment.
    pub commit_id: u64,

    /// Delegation record tying this account to the ER context.
    pub delegation_record: Pubkey,

    /// Hash of replay/data-availability pointer bytes.
    pub da_pointer_hash: [u8; 32],

    /// Hash of lamports, owner, and data_hash.
    pub account_state_hash: [u8; 32],

    /// Hash of full account data.
    pub data_hash: [u8; 32],

    /// Lamports committed by the operator.
    pub lamports: u64,

    /// Owner committed by the operator.
    pub owner: Pubkey,

    /// Hash binding operator, account, commit id, delegation, DA, and state.
    pub state_commitment_hash: [u8; 32],

    /// Registry account used when this commitment was posted.
    pub verifier_registry: Pubkey,

    /// Copy of `VerifierRegistry.registry_revision` at post time.
    pub verifier_registry_revision: u64,

    /// Monotonic id for this approval/challenge window.
    pub challenge_window_id: u64,

    /// Slot when the commitment was posted.
    pub posted_slot: u64,

    /// Slot when verifier selection and the challenge window started.
    pub activation_slot: u64,

    /// Slot when approval/challenge window closes.
    pub challenge_window_end_slot: u64,

    /// Number of unique selected verifiers that approved.
    pub approval_count: u16,

    /// Threshold copied from ProtocolConfig when the commitment is posted.
    pub approval_threshold: u16,

    /// Active Challenge account, if any.
    pub active_challenge: Option<Pubkey>,

    /// Which opened state finalization must use after dispute resolution.
    pub resolved_state_source: Option<u8>,

    /// ER slot that produced this commitment, when available.
    pub er_slot: Option<u64>,

    /// Aligns selected verifier elements after the Vec length prefix.
    pub _pad_before_selected_verifiers: [u8; 7],

    /// Verifiers selected by round-robin for this commitment.
    #[flexible = 2]
    pub selected_verifiers: Vec<SelectedVerifier>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[fixed_offset_layout(buffer_offset = 2)]
pub struct SelectedVerifier {
    /// Selected verifier identity.
    pub verifier_identity: Pubkey,

    /// Whether this verifier has approved this commitment.
    pub approved: bool,

    /// Keeps each Vec element aligned for fixed-layout decoding.
    pub _pad_after_approved: [u8; 7],
}

impl PendingCommitment {
    pub const DISCRIMINATOR: [u8; 8] = *b"v2pend00";
}
