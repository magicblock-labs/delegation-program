use wheels::{layout::Decodable, variable_offset_layout};

use crate::solana_program::program_error::ProgramError;

pub const VERIFIER_REGISTRY_ACTION_ADD: u8 = 1;
pub const VERIFIER_REGISTRY_ACTION_REMOVE: u8 = 2;

#[derive(Clone, Debug, PartialEq, Eq)]
#[variable_offset_layout(buffer_offset = unaligned)]
pub struct UpdateVerifierRegistryArgs {
    pub action: u8,
    pub weight: u64,
}

impl UpdateVerifierRegistryArgs {
    pub fn try_from_bytes(data: &[u8]) -> Result<Self, ProgramError> {
        let view = <Self as Decodable>::decode(data)
            .map_err(super::super::state::layout_error_to_program_error)?;

        Ok(Self {
            action: view.action(),
            weight: view.weight(),
        })
    }
}
