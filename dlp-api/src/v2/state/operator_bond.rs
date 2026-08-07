use wheels::{
    fixed_offset_layout,
    layout::{Decodable, Encodable},
};

use crate::{compat::Pubkey, solana_program::program_error::ProgramError};

pub const OPERATOR_STATUS_ACTIVE: u8 = 1;
pub const OPERATOR_STATUS_EXITING: u8 = 2;
pub const OPERATOR_STATUS_SLASHED: u8 = 3;
pub const OPERATOR_STATUS_JAILED: u8 = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
#[fixed_offset_layout(buffer_offset = 0)]
pub struct OperatorBond {
    pub operator_identity: Pubkey,
    pub stake_lamports: u64,
    pub locked_lamports: u64,
    pub status: u8,
    pub withdraw_requested_slot: Option<u64>,
}

impl OperatorBond {
    pub const DISCRIMINATOR: [u8; 8] = *b"v2opbond";
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
            operator_identity: *view.operator_identity(),
            stake_lamports: view.stake_lamports(),
            locked_lamports: view.locked_lamports(),
            status: view.status(),
            withdraw_requested_slot: view.withdraw_requested_slot(),
        })
    }
}
