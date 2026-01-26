use num_enum::TryFromPrimitive;
use pinocchio::ProgramResult;
use strum::IntoStaticStr;

use crate::error::DlpError;

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, TryFromPrimitive, IntoStaticStr)]
#[rustfmt::skip]
pub enum DlpInstruction {
    ///
    /// Delegate group: [101, 110] => 10 slots
    ///
    Delegate = 101,
    DelegateWithAnyValidator = 102,

    ///
    /// Commit group: [111, 120] => 10 slots
    ///
    Commit = 111,
    CommitFromBuffer = 112,
    CommitFinalize = 113,
    CommitFinalizeFromBuffer = 114,
    HyperCommitFinalize = 115,

    ///
    /// Finalize group: [121, 130] => 10 slots
    ///
    Finalize = 121,

    ///
    /// Undelegate group: [131, 140] => 10 slots
    ///
    Undelegate = 131,
    UndelegateConfinedAccount = 132,

    ///
    /// User group: [141, 150] => 10 slots
    ///
    CallHandler = 141,

    ///
    /// Vaults group: [151, 160] => 10 slots
    ///
    InitProtocolFeesVault = 151,
    ProtocolClaimFees = 152,
    InitValidatorFeesVault = 153,
    ValidatorClaimFees = 154,
    CloseValidatorFeesVault = 155,

    ///
    /// Misc group: [161, 180] => 20 slots
    ///
    WhitelistValidatorForProgram = 161,
    TopUpEphemeralBalance = 162,
    DelegateEphemeralBalance = 163,
    CloseEphemeralBalance = 164,
}

impl DlpInstruction {
    pub fn to_vec(self) -> Vec<u8> {
        let num = self as u64;
        num.to_le_bytes().to_vec()
    }

    pub fn name(&self) -> &'static str {
        self.into()
    }

    const fn index(self) -> usize {
        self as usize
    }
}

#[inline(always)]
fn instruction_not_found(
    _: &[pinocchio::AccountView],
    _: &[u8],
) -> ProgramResult {
    Err(DlpError::InstructionNotFound.into())
}

pub type IxHandler = fn(&[pinocchio::AccountView], &[u8]) -> ProgramResult;

#[rustfmt::skip]
pub const IX_TABLE: [IxHandler; 256] = {
    use super::processor::*;

    let mut table = [instruction_not_found as IxHandler; 256];

    use DlpInstruction::*;

    // Delegate group
    table[Delegate.index()]                 = process_delegate;
    table[DelegateWithAnyValidator.index()] = process_delegate_with_any_validator;

    // Commit group
    table[CommitFinalize.index()]           = process_commit_finalize;
    table[CommitFinalizeFromBuffer.index()] = process_commit_finalize_from_buffer;
    table[HyperCommitFinalize.index()]       = process_hyper_commit_finalize;

    table
};
