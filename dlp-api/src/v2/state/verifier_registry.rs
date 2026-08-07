use wheels::{
    fixed_offset_layout,
    layout::{Decodable, Encodable},
    variable_offset_layout,
};

use crate::{compat::Pubkey, solana_program::program_error::ProgramError};

#[derive(Clone, Debug, PartialEq, Eq)]
#[fixed_offset_layout]
pub struct VerifierRegistryEntry {
    pub verifier_identity: Pubkey,
    pub verifier_bond: Pubkey,
    pub weight: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VerifierRegistry {
    pub registry_revision: u64,
    pub entries: Vec<VerifierRegistryEntry>,
}

#[variable_offset_layout(buffer_offset = unaligned)]
struct VerifierRegistryLayout {
    registry_revision: u64,
    #[flexible = 4]
    entries: Vec<VerifierRegistryEntry>,
}

impl VerifierRegistry {
    pub const DISCRIMINATOR: [u8; 8] = *b"v2vreg00";
    pub const EMPTY_SPACE: usize = 8 + VerifierRegistryLayout::DATA_LEN_RANGE.0;

    pub fn size_with_discriminator(&self) -> usize {
        Self::EMPTY_SPACE + self.entries.len() * VerifierRegistryEntry::DATA_LEN
    }

    pub fn to_bytes_with_discriminator(
        &self,
        data: &mut [u8],
    ) -> Result<(), ProgramError> {
        let payload = super::utils::payload_with_discriminator_mut(
            &Self::DISCRIMINATOR,
            data,
        )?;
        let layout = VerifierRegistryLayout {
            registry_revision: self.registry_revision,
            entries: self.entries.clone(),
        };
        layout
            .encode_to(payload)
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
        let view = <VerifierRegistryLayout as Decodable>::decode(payload)
            .map_err(super::utils::layout_error_to_program_error)?;

        Ok(Self {
            registry_revision: view.registry_revision(),
            entries: view
                .entries()
                .iter()
                .map(|entry| VerifierRegistryEntry {
                    verifier_identity: *entry.verifier_identity(),
                    verifier_bond: *entry.verifier_bond(),
                    weight: entry.weight(),
                })
                .collect(),
        })
    }
}
