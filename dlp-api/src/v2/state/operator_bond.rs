use wheels::fixed_offset_layout;

use crate::compat::Pubkey;

#[derive(Clone, Debug, PartialEq, Eq)]
#[fixed_offset_layout(buffer_offset = 0)]
pub struct OperatorBond {
    /// Account type marker.
    pub discriminator: [u8; 8],

    /// Canonical PDA bump for this account.
    pub bump: u8,

    /// Operator identity allowed to post commitments through this bond.
    pub operator_identity: Pubkey,

    /// Slashable operator stake held in this account.
    /// CHECKPOINT: the staking asset is SOL or BLOCK?
    /// If this changes to BLOCK, this field will need to point at token-account
    /// accounting instead of native lamports.
    pub stake_lamports: u64,

    /// Stake reserved by active commitments.
    /// CHECKPOINT: the staking asset is SOL or BLOCK?
    pub locked_lamports: u64,

    /// Current operator lifecycle state, stored as `OperatorStatus::value()`.
    pub status: u8,

    /// Slot when withdrawal was requested, if the operator is exiting.
    pub withdraw_requested_slot: Option<u64>,
}

impl OperatorBond {
    pub const DISCRIMINATOR: [u8; 8] = *b"v2opbond";
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperatorStatus {
    Active = 1,
    Exiting = 2,
    Slashed = 3,
    Jailed = 4,
}

impl OperatorStatus {
    pub const fn value(self) -> u8 {
        self as u8
    }
}
