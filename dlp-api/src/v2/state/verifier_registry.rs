use crate::{
    compat::{
        borsh::{BorshDeserialize, BorshSerialize},
        Pubkey,
    },
    solana_program::program_error::ProgramError,
};

#[derive(Clone, Debug, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub struct VerifierRegistryEntry {
    pub verifier_identity: Pubkey,
    pub verifier_bond: Pubkey,
    pub weight: u64,
}

impl VerifierRegistryEntry {
    pub const SPACE: usize = 32 + 32 + 8;
}

#[derive(
    Clone, Debug, Default, BorshSerialize, BorshDeserialize, PartialEq, Eq,
)]
pub struct VerifierRegistry {
    pub registry_revision: u64,
    pub entries: Vec<VerifierRegistryEntry>,
}

impl VerifierRegistry {
    pub const DISCRIMINATOR: [u8; 8] = *b"v2vreg00";
    pub const EMPTY_SPACE: usize =
        8 + Self::REGISTRY_REVISION_SPACE + Self::VEC_LEN_SPACE;

    const REGISTRY_REVISION_SPACE: usize = 8;
    const VEC_LEN_SPACE: usize = 4;

    pub fn size_with_discriminator(&self) -> usize {
        Self::EMPTY_SPACE + self.entries.len() * VerifierRegistryEntry::SPACE
    }

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
