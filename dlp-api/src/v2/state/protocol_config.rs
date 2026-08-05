use wheels::{
    layout::{Decodable, Encodable},
    variable_offset_layout,
};

use crate::{compat::Pubkey, solana_program::program_error::ProgramError};

#[derive(Clone, Debug, PartialEq, Eq)]
#[variable_offset_layout(buffer_offset = unaligned)]
pub struct ProtocolConfig {
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
    pub const SPACE: usize = 8 + Self::DATA_LEN;

    pub fn to_bytes_with_discriminator(
        &self,
        data: &mut [u8],
    ) -> Result<(), ProgramError> {
        let payload = super::utils::payload_with_discriminator_mut(
            &Self::DISCRIMINATOR,
            data,
        )?;
        self.encode_to(payload)
            .map_err(super::utils::layout_error_to_program_error)?;
        Ok(())
    }

    pub fn try_from_bytes_with_discriminator(
        data: &[u8],
    ) -> Result<Self, ProgramError> {
        let payload = super::utils::payload_with_discriminator(
            &Self::DISCRIMINATOR,
            data,
        )?;
        let view = <Self as Decodable>::decode(payload)
            .map_err(super::utils::layout_error_to_program_error)?;

        Ok(Self {
            authority: *view.authority(),
            paused: view.paused(),
            vrf_program: *view.vrf_program(),
            vrf_config: *view.vrf_config(),
            resolver: *view.resolver(),
            protocol_fee_vault: *view.protocol_fee_vault(),
            min_operator_bond: view.min_operator_bond(),
            min_verifier_bond: view.min_verifier_bond(),
            min_challenger_stake: view.min_challenger_stake(),
            challenge_window_slots: view.challenge_window_slots(),
            operator_response_timeout_slots: view
                .operator_response_timeout_slots(),
            challenger_reveal_timeout_slots: view
                .challenger_reveal_timeout_slots(),
            payout_timelock_slots: view.payout_timelock_slots(),
            selected_verifier_count: view.selected_verifier_count(),
            approval_threshold: view.approval_threshold(),
            max_window_extensions: view.max_window_extensions(),
            match_penalty_bps: view.match_penalty_bps(),
        })
    }
}
