use crate::{
    compat::{
        borsh::{BorshDeserialize, BorshSerialize},
        Pubkey,
    },
    solana_program::program_error::ProgramError,
};

#[derive(Clone, Debug, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
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
    pub const SPACE: usize = 8
        + 5 * Self::PUBKEY_SPACE
        + Self::BOOL_SPACE
        + 7 * Self::U64_SPACE
        + 4 * Self::U16_SPACE;

    const PUBKEY_SPACE: usize = 32;
    const BOOL_SPACE: usize = 1;
    const U64_SPACE: usize = 8;
    const U16_SPACE: usize = 2;

    pub fn to_bytes_with_discriminator<W>(
        &self,
        writer: &mut W,
    ) -> Result<(), ProgramError>
    where
        W: std::io::Write,
    {
        super::utils::write_with_discriminator(
            &Self::DISCRIMINATOR,
            self,
            writer,
        )
    }

    pub fn try_from_bytes_with_discriminator(
        data: &[u8],
    ) -> Result<Self, ProgramError> {
        super::utils::try_from_bytes_with_discriminator(
            &Self::DISCRIMINATOR,
            data,
        )
    }
}
