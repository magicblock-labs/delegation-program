use wheels::fixed_offset_layout;

use crate::compat::Pubkey;

pub const OPERATOR_STATUS_ACTIVE: u8 = 1;
pub const OPERATOR_STATUS_EXITING: u8 = 2;
pub const OPERATOR_STATUS_SLASHED: u8 = 3;
pub const OPERATOR_STATUS_JAILED: u8 = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
#[fixed_offset_layout(buffer_offset = 0)]
pub struct OperatorBond {
    /// Account type marker.
    pub discriminator: [u8; 8],

    /// Operator identity allowed to post commitments through this bond.
    pub operator_identity: Pubkey,

    /// Slashable operator stake held in this account.
    pub stake_lamports: u64,

    /// Stake reserved by active commitments.
    pub locked_lamports: u64,

    /// Current operator lifecycle state.
    pub status: u8,

    /// Slot when withdrawal was requested, if the operator is exiting.
    pub withdraw_requested_slot: Option<u64>,
}

impl OperatorBond {
    pub const DISCRIMINATOR: [u8; 8] = *b"v2opbond";
}
