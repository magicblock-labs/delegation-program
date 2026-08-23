use wheels::fixed_offset_layout;

use crate::compat::Pubkey;

pub const VERIFIER_STATUS_ACTIVE: u8 = 1;
pub const VERIFIER_STATUS_EXITING: u8 = 2;
pub const VERIFIER_STATUS_SLASHED: u8 = 3;
pub const VERIFIER_STATUS_JAILED: u8 = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
#[fixed_offset_layout(buffer_offset = 0)]
pub struct VerifierBond {
    /// Account type marker.
    pub discriminator: [u8; 8],

    /// Verifier identity allowed to approve commitments through this bond.
    pub verifier_identity: Pubkey,

    /// Slashable verifier stake held in this account.
    pub stake_lamports: u64,

    /// Current verifier lifecycle state.
    pub status: u8,

    /// Slot when the verifier registered.
    pub registered_slot: u64,

    /// Slot when withdrawal was requested, if the verifier is exiting.
    pub withdraw_requested_slot: Option<u64>,
}

impl VerifierBond {
    pub const DISCRIMINATOR: [u8; 8] = *b"v2vrbond";
}
