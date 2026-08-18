use wheels::fixed_offset_layout;

use crate::compat::Pubkey;

#[derive(Clone, Debug, PartialEq, Eq)]
#[fixed_offset_layout(buffer_offset = 0)]
pub struct ProtocolConfig {
    pub discriminator: [u8; 8],
    pub authority: Pubkey,
    pub paused: bool,

    pub vrf_program: Pubkey,
    pub vrf_config: Pubkey,
    pub resolver: Pubkey,
    pub protocol_fee_vault: Pubkey,

    pub min_operator_bond: u64,
    pub min_verifier_bond: u64,
    pub min_challenger_stake: u64,

    pub challenge_window_slots: u64,
    pub operator_response_timeout_slots: u64,
    pub challenger_reveal_timeout_slots: u64,
    pub payout_timelock_slots: u64,

    pub selected_verifier_count: u16,
    pub approval_threshold: u16,
    pub max_window_extensions: u16,
    pub match_penalty_bps: u16,
}

impl ProtocolConfig {
    pub const DISCRIMINATOR: [u8; 8] = *b"v2cfg000";
    pub const SPACE: usize = Self::DATA_LEN;
}
