use wheels::fixed_offset_layout;

use crate::compat::Pubkey;

pub const CHALLENGE_STATUS_AWAITING_REVEAL: u8 = 1;
pub const CHALLENGE_STATUS_AWAITING_RESOLVER: u8 = 2;
pub const CHALLENGE_STATUS_TERMINAL: u8 = 3;

pub const CHALLENGE_OUTCOME_NONE: u8 = 0;
pub const CHALLENGE_OUTCOME_INVALID_REVEAL: u8 = 1;
pub const CHALLENGE_OUTCOME_MATCHING_STATE_CHALLENGER_PENALIZED: u8 = 2;
pub const CHALLENGE_OUTCOME_OPERATOR_CORRECT_CHALLENGER_SLASHED: u8 = 3;
pub const CHALLENGE_OUTCOME_CHALLENGER_CORRECT_OPERATOR_SLASHED: u8 = 4;

/// PDA: `["challenge", account, commit_id, challenger]`.
/// Created by `RaiseChallenge`.
/// Closed by `CloseTerminalAccounts` after terminal challenge outcome.
#[derive(Clone, Debug, PartialEq, Eq)]
#[fixed_offset_layout(buffer_offset = 0)]
pub struct Challenge {
    /// Account type marker.
    pub discriminator: [u8; 8],

    /// Current challenge lifecycle state.
    pub status: u8,

    /// Terminal outcome, or `CHALLENGE_OUTCOME_NONE` before resolution.
    pub outcome: u8,

    /// Keeps the following fixed-width fields 8-byte aligned.
    pub _pad_after_outcome: [u8; 6],

    /// PendingCommitment being challenged.
    pub pending_commitment: Pubkey,

    /// Challenger that locked stake and owns the reveal.
    pub challenger_identity: Pubkey,

    /// State commitment hash copied from the pending commitment at raise time.
    pub state_commitment_hash: [u8; 32],

    /// Salted hash binding the challenger state to this challenge.
    pub challenge_hash: [u8; 32],

    /// Challenger-revealed account lamports. Zero until reveal.
    pub challenger_lamports: u64,

    /// Challenger-revealed account owner. Default pubkey until reveal.
    pub challenger_owner: Pubkey,

    /// Challenger-revealed account data hash. Zero until reveal.
    pub challenger_data_hash: [u8; 32],

    /// Challenger StateBuffer PDA used for reveal. Default pubkey until reveal.
    pub challenger_state_buffer: Pubkey,

    /// Lamports locked in this challenge account.
    pub challenger_stake_lamports: u64,

    /// Slot when the challenge was raised.
    pub raised_slot: u64,

    /// Slot after which an unrevealed challenge can be timed out.
    pub reveal_deadline_slot: u64,
}

impl Challenge {
    pub const DISCRIMINATOR: [u8; 8] = *b"v2chal00";
}
