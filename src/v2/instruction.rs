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
    DelegateInline = 103,
    DelegateInline32 = 104,

    ///
    /// Commit group: [111, 140] => 30 slots
    ///
    /// From users perspective, there are only 3 categories of commits:
    ///     - commit
    ///     - commit-inline
    ///     - commit-finalize
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

    // CommitInline32 = 113,
    // CommitInline32FromBuffer = 114,
    // CommitInline32Resize = 115,
    // CommitInline32ResizeFromBuffer = 116,

    // CommitFinalizeInline32 = 119,
    // CommitFinalizeInline32FromBuffer = 120,
    // CommitFinalizeInline32Resize = 121,
    // CommitFinalizeInline32ResizeFromBuffer = 122,

    ///
    /// Finalize group: [141, 150] => 10 slots
    ///
    Finalize = 141,

    ///
    /// Undelegate group: [151, 160] => 10 slots
    ///
    Undelegate = 151,
    UndelegateConfinedAccount = 152,

    ///
    /// User group: [161, 170] => 10 slots
    ///
    CallHandler = 161,

    ///
    /// Vaults group: [171, 180] => 10 slots
    ///
    InitProtocolFeesVault = 171,
    ProtocolClaimFees = 172,
    InitValidatorFeesVault = 173,
    ValidatorClaimFees = 174,
    CloseValidatorFeesVault = 175,

    ///
    /// Misc group: [181, 200] => 20 slots
    ///
    WhitelistValidatorForProgram = 181,
    TopUpEphemeralBalance = 182,
    DelegateEphemeralBalance = 183,
    CloseEphemeralBalance = 184,
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
    table[Commit.index()]                   = process_commit;
    table[CommitFromBuffer.index()]         = process_commit_from_buffer;
    table[CommitFinalize.index()]           = process_commit_finalize;
    table[CommitFinalizeFromBuffer.index()] = process_commit_finalize_from_buffer;
    table[CommitFinalizeInline.index()]       = |accounts, data| {
        process_commit_finalize_inline(accounts, data.as_ptr(), data.len())
    };
    table[CommitFinalizeInlineResize.index()]       = |accounts, data| {
        process_commit_finalize_inline_resize(accounts, data.as_ptr(), data.len())
    };

    table
};
