use wheels::fixed_offset_layout;

use crate::compat::Pubkey;

#[derive(Clone, Debug, PartialEq, Eq)]
#[fixed_offset_layout(buffer_offset = 0)]
pub struct VerifierBond {
    /// Account type marker.
    pub discriminator: [u8; 8],

    /// Canonical PDA bump for this account.
    pub bump: u8,

    /// Verifier identity allowed to approve commitments through this bond.
    pub verifier_identity: Pubkey,

    /// Slashable verifier stake held in this account.
    /// CHECKPOINT: the staking asset is SOL or BLOCK?
    /// If this changes to BLOCK, this field will need to point at token-account
    /// accounting instead of native lamports.
    pub stake_lamports: u64,

    /// Current verifier lifecycle state, stored as `VerifierStatus::value()`.
    pub status: u8,

    /// Slot when the verifier registered.
    /// CHECKPOINT: keep this only if future verifier eligibility rules compare
    /// the current slot with this registration slot, such as requiring a newly
    /// registered verifier to wait before it can be selected or approve.
    pub registered_slot: u64,

    /// Slot when withdrawal was requested, if the verifier is exiting.
    pub withdraw_requested_slot: Option<u64>,
}

impl VerifierBond {
    pub const DISCRIMINATOR: [u8; 8] = *b"v2vrbond";
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerifierStatus {
    Active = 1,
    Exiting = 2,
    Slashed = 3,
    Jailed = 4,
}

impl VerifierStatus {
    pub const fn value(self) -> u8 {
        self as u8
    }
}
