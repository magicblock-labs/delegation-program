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

    /// Canonical PDA bump for this account.
    pub bump: u8,

    /// Round-robin start cursor used by the next commitment selection.
    pub next_selection_index: u64,

    /// All registered verifiers DLP can select from.
    #[extendable = 2]
    pub entries: Vec<VerifierRegistryEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[fixed_offset_layout(buffer_offset = 3)]
pub struct VerifierRegistryEntry {
    /// Verifier identity selectable by DLP.
    pub verifier_identity: Pubkey,

    /// Bond account proving this verifier has active stake.
    pub verifier_bond: Pubkey,

    /// Selection weight. Keep as 1 until weighted selection is implemented.
    pub weight: u64,
}

impl VerifierRegistry {
    pub const DISCRIMINATOR: [u8; 8] = *b"v2vreg00";
}
