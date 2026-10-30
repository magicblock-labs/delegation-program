use wheels::{
    fixed_offset_layout, layout::VariableLayoutSlice, DataLayoutError,
};

use crate::{compat::Pubkey, error::DlpError, DelegationSeed};

/// PDA: `["delegation-state", delegated_account]`.
/// Created by `Delegate`; closed when the account is undelegated.
#[derive(Clone, Debug, PartialEq, Eq)]
#[fixed_offset_layout(buffer_offset = 0)]
pub struct DelegationState {
    /// Account type marker.
    pub discriminator: [u8; 8],

    /// Canonical PDA bump for this account.
    pub bump: u8,

    /// Account controlled by this delegation state.
    pub delegated_account: Pubkey,

    /// Stable cluster authority for this delegation.
    pub delegation_cluster: Pubkey,

    /// Program that owned the account before delegation.
    pub original_owner: Pubkey,

    /// Account that paid to create this delegation state.
    pub rent_payer: Pubkey,

    /// Slot in which this delegation was created.
    pub delegation_slot: u64,

    /// Account lamports at delegation or after the latest finalization.
    pub lamports: u64,

    /// Latest finalized commit id applied to the delegated account.
    pub last_commit_seq: u64,

    /// Who requested undelegation, stored as `UndelegationRequester::value()`.
    pub undelegation_requester: u8,

    /// Bump used with the stored seeds for an off-curve delegated account.
    pub delegated_account_bump: u8,

    /// Number of owner-program seeds encoded in `encoded_seeds`.
    pub seed_count: u8,

    /// Self-delimiting `DelegationSeed` values used to derive the account.
    #[extendable = 2]
    pub encoded_seeds: Vec<u8>,
}

impl DelegationState {
    pub const DISCRIMINATOR: [u8; 8] = *b"v2delst0";
    pub const MAX_SEEDS: usize = 8;
    pub const MAX_SEED_LEN: usize = 32;
    pub const ENCODED_SEEDS_LEN_HEADER_LEN: usize = 2;

    pub fn data_len_from_encoded_seeds_len(
        encoded_seeds_len: usize,
    ) -> Result<usize, DlpError> {
        Self::MIN_DATA_LEN
            .checked_add(Self::ENCODED_SEEDS_LEN_HEADER_LEN)
            .and_then(|len| len.checked_add(encoded_seeds_len))
            .ok_or(DlpError::Overflow)
    }
}

impl<'a> DelegationStateView<'a> {
    pub fn seeds(
        &self,
    ) -> Result<VariableLayoutSlice<'a, DelegationSeed>, DataLayoutError> {
        VariableLayoutSlice::new(
            self.encoded_seeds().as_slice(),
            self.seed_count() as usize,
        )
    }
}

/// Identifies who requested that a delegated account be undelegated.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UndelegationRequester {
    None = 0,
    Validator = 1,
    OwnerProgram = 2,
}

impl UndelegationRequester {
    pub const fn value(self) -> u8 {
        self as u8
    }
}
