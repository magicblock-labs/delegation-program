use num_enum::TryFromPrimitive;
use pinocchio::{error::ProgramError, ProgramResult};
use strum::IntoStaticStr;

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
}

pub fn v2_process_instruction(
    accounts: &[pinocchio::AccountView],
    data: &[u8],
) -> ProgramResult {
    let (ix, data) = data.split_at(8);

    let ix = match DlpInstruction::try_from(ix[0]) {
        Ok(discriminator) => discriminator,
        Err(_) => {
            pinocchio_log::log!("Failed to read and parse discriminator");
            return Err(pinocchio::error::ProgramError::InvalidInstructionData);
        }
    };

    use super::processor::*;

    let coming_soon = || {
        solana_program::msg!("Instruction {:#?} not yet implemented", ix);
        return Err(ProgramError::InvalidInstructionData);
    };

    match ix {
        DlpInstruction::Delegate => process_delegate(accounts, data),
        DlpInstruction::DelegateWithAnyValidator => {
            process_delegate_with_any_validator(accounts, data)
        }
        DlpInstruction::Commit => coming_soon(),
        DlpInstruction::CommitFromBuffer => coming_soon(),
        DlpInstruction::CommitFinalize => coming_soon(),
        DlpInstruction::CommitFinalizeFromBuffer => coming_soon(),
        DlpInstruction::Finalize => coming_soon(),
        DlpInstruction::Undelegate => coming_soon(),
        DlpInstruction::UndelegateConfinedAccount => coming_soon(),
        DlpInstruction::CallHandler => coming_soon(),
        DlpInstruction::InitProtocolFeesVault => coming_soon(),
        DlpInstruction::ProtocolClaimFees => coming_soon(),
        DlpInstruction::InitValidatorFeesVault => coming_soon(),
        DlpInstruction::ValidatorClaimFees => coming_soon(),
        DlpInstruction::CloseValidatorFeesVault => coming_soon(),
        DlpInstruction::WhitelistValidatorForProgram => coming_soon(),
        DlpInstruction::TopUpEphemeralBalance => coming_soon(),
        DlpInstruction::DelegateEphemeralBalance => coming_soon(),
        DlpInstruction::CloseEphemeralBalance => coming_soon(),
    }
}
