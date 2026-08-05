use wheels::{
    layout::{Decodable, Encodable},
    variable_offset_layout,
};

use crate::{compat::Pubkey, solana_program::program_error::ProgramError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifierRegistryEntry {
    pub verifier_identity: Pubkey,
    pub verifier_bond: Pubkey,
    pub weight: u64,
}

impl VerifierRegistryEntry {
    pub const SPACE: usize = 32 + 32 + 8;

    fn to_wire(&self) -> [u8; 72] {
        let mut bytes = [0; 72];
        bytes[..32].copy_from_slice(self.verifier_identity.as_ref());
        bytes[32..64].copy_from_slice(self.verifier_bond.as_ref());
        bytes[64..72].copy_from_slice(&self.weight.to_le_bytes());
        bytes
    }

    fn from_wire(bytes: &[u8; 72]) -> Self {
        let mut verifier_identity = [0; 32];
        verifier_identity.copy_from_slice(&bytes[..32]);

        let mut verifier_bond = [0; 32];
        verifier_bond.copy_from_slice(&bytes[32..64]);

        let mut weight = [0; 8];
        weight.copy_from_slice(&bytes[64..72]);

        Self {
            verifier_identity: verifier_identity.into(),
            verifier_bond: verifier_bond.into(),
            weight: u64::from_le_bytes(weight),
        }
    }
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
    entries: Vec<u8>,
}

impl VerifierRegistry {
    pub const DISCRIMINATOR: [u8; 8] = *b"v2vreg00";
    pub const EMPTY_SPACE: usize = 8 + VerifierRegistryLayout::DATA_LEN_RANGE.0;

    pub fn size_with_discriminator(&self) -> usize {
        Self::EMPTY_SPACE + self.entries.len() * 72
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
            entries: self
                .entries
                .iter()
                .flat_map(|entry| entry.to_wire())
                .collect(),
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
            entries: entries_from_wire(view.entries())?,
        })
    }
}

fn entries_from_wire(
    bytes: &[u8],
) -> Result<Vec<VerifierRegistryEntry>, ProgramError> {
    if bytes.len() % VerifierRegistryEntry::SPACE != 0 {
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(bytes
        .chunks_exact(VerifierRegistryEntry::SPACE)
        .map(|chunk| {
            let mut entry = [0; 72];
            entry.copy_from_slice(chunk);
            VerifierRegistryEntry::from_wire(&entry)
        })
        .collect())
}
