use wheels::fixed_offset_layout;

use crate::compat::Pubkey;

#[derive(Clone, Debug, PartialEq, Eq)]
#[fixed_offset_layout(buffer_offset = 2)]
pub struct VerifierRegistryEntry {
    pub verifier_identity: Pubkey,
    pub verifier_bond: Pubkey,
    pub weight: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[fixed_offset_layout(buffer_offset = 0)]
pub struct VerifierRegistry {
    pub discriminator: [u8; 8],
    pub registry_revision: u64,
    #[flexible = 2]
    pub entries: Vec<VerifierRegistryEntry>,
}

impl VerifierRegistry {
    pub const DISCRIMINATOR: [u8; 8] = *b"v2vreg00";
    pub const EMPTY_SPACE: usize = Self::MIN_DATA_LEN;
}

impl Default for VerifierRegistry {
    fn default() -> Self {
        Self {
            discriminator: Self::DISCRIMINATOR,
            registry_revision: 0,
            entries: Vec::new(),
        }
    }
}
