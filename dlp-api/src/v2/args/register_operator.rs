use wheels::{layout::Decodable, variable_offset_layout};

use crate::solana_program::program_error::ProgramError;

#[derive(Clone, Debug, PartialEq, Eq)]
#[variable_offset_layout(buffer_offset = unaligned)]
pub struct RegisterOperatorArgs {
    pub amount_lamports: u64,
}

impl RegisterOperatorArgs {
    pub fn try_from_bytes(data: &[u8]) -> Result<Self, ProgramError> {
        let view = <Self as Decodable>::decode(data)
            .map_err(super::super::state::layout_error_to_program_error)?;

        Ok(Self {
            amount_lamports: view.amount_lamports(),
        })
    }
}
