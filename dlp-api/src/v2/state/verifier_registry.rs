use wheels::fixed_offset_layout;

use crate::compat::Pubkey;

/// PDA: `["verifier-registry"]`.
/// Created by `InitProtocolConfig`; updated by `UpdateVerifierRegistry`.
/// Not normally closed.
#[derive(Clone, Debug, PartialEq, Eq)]
#[fixed_offset_layout(buffer_offset = 0)]
pub struct VerifierRegistry {
    /// Account type marker.
    pub discriminator: [u8; 8],

    /// Increments every time `entries` changes.
    ///
    /// A pending commitment stores this value before requesting VRF. The VRF
    /// callback must see the same value before it can select verifiers.
    pub registry_revision: u64,

    /// All registered verifiers DLP can select from.
    #[flexible = 2]
    pub entries: Vec<VerifierRegistryEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[fixed_offset_layout(buffer_offset = 2)]
pub struct VerifierRegistryEntry {
    /// Verifier identity selectable by VRF.
    pub verifier_identity: Pubkey,

    /// Bond account proving this verifier has active stake.
    pub verifier_bond: Pubkey,

    /// Selection weight. Keep as 1 for equal-weight selection.
    pub weight: u64,
}

impl VerifierRegistry {
    pub const DISCRIMINATOR: [u8; 8] = *b"v2vreg00";
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
