use wheels::variable_offset_layout;

use crate::compat::Pubkey;

#[derive(Clone, Debug, PartialEq, Eq)]
#[variable_offset_layout(buffer_offset = 1)]
pub struct InitProtocolConfigArgs {
    pub resolver: Pubkey,

    pub min_operator_bond: u64,
    pub min_verifier_bond: u64,
    pub min_challenger_stake: u64,

    pub challenge_window_slots: u64,
    pub operator_response_timeout_slots: u64,
    pub challenger_reveal_timeout_slots: u64,
    pub payout_timelock_slots: u64,

    pub verifiers_per_commitment: u16,
    pub approval_threshold: u16,
    pub max_window_extensions: u16,
    pub match_penalty_bps: u16,
}
