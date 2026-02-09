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
    /// Commit group: [111, 130] => 20 slots
    ///
    Commit = 111,
    CommitFromBuffer = 112,
    CommitInline = 113,
    CommitInlineFromBuffer = 114,
    CommitInlineResize = 115,
    CommitInlineResizeFromBuffer = 116,

    CommitFinalize = 117,
    CommitFinalizeFromBuffer = 118,
    CommitFinalizeInline = 119,
    CommitFinalizeInlineFromBuffer = 120,
    CommitFinalizeInlineResize = 121,
    CommitFinalizeInlineResizeFromBuffer = 122,

    ///
    /// Finalize group: [131, 140] => 10 slots
    ///
    Finalize = 131,

    ///
    /// Undelegate group: [141, 150] => 10 slots
    ///
    Undelegate = 141,
    UndelegateConfinedAccount = 132,

    ///
    /// User group: [151, 160] => 10 slots
    ///
    CallHandler = 151,

    ///
    /// Vaults group: [161, 170] => 10 slots
    ///
    InitProtocolFeesVault = 161,
    ProtocolClaimFees = 162,
    InitValidatorFeesVault = 163,
    ValidatorClaimFees = 164,
    CloseValidatorFeesVault = 165,

    ///
    /// Misc group: [171, 190] => 20 slots
    ///
    WhitelistValidatorForProgram = 171,
    TopUpEphemeralBalance = 172,
    DelegateEphemeralBalance = 173,
    CloseEphemeralBalance = 174,
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
    //unsafe { std::hint::unreachable_unchecked() }
    Err(DlpError::InstructionNotFound.into())
}

pub type Processor = fn(&[pinocchio::AccountView], &[u8]) -> ProgramResult;

#[rustfmt::skip]
pub const IX_TABLE: [Processor; 256] = {
    use super::processor::*;

    let mut table = [instruction_not_found as Processor; 256];

    use DlpInstruction::*;

    // Delegate group
    table[Delegate.index()]                 = process_delegate;
    table[DelegateWithAnyValidator.index()] = process_delegate_with_any_validator;

    // Commit group
    table[CommitFinalize.index()]           = process_commit_finalize;
    table[CommitFinalizeFromBuffer.index()] = process_commit_finalize_from_buffer;
    table[CommitFinalizeInline.index()]       = |accounts, data| {
        process_commit_finalize_inline(accounts, data.as_ptr(), data.len())
    };

    table
};
