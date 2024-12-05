use num_enum::TryFromPrimitive;
use solana_program::program_error::ProgramError;

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, TryFromPrimitive)]
#[rustfmt::skip]
pub enum DlpDiscriminant {
    Delegate = 0,
    CommitState = 1,
    Finalize = 2,
    Undelegate = 3,
    AllowUndelegate = 4,
    InitFeesVault = 5,
    InitValidatorFeesVault = 6,
    ValidatorClaimFees = 7,
    WhitelistValidatorForProgram = 8,
    TopUpEphemeralBalance = 9,
    DelegateEphemeralBalance = 10,
    CloseEphemeralBalance = 11
}

impl DlpDiscriminant {
    pub fn to_vec(self) -> Vec<u8> {
        let num = self as u64;
        num.to_le_bytes().to_vec()
    }
}

impl TryFrom<[u8; 8]> for DlpDiscriminant {
    type Error = ProgramError;
    fn try_from(bytes: [u8; 8]) -> Result<Self, Self::Error> {
        match bytes[0] {
            0x0 => Ok(DlpDiscriminant::Delegate),
            0x1 => Ok(DlpDiscriminant::CommitState),
            0x2 => Ok(DlpDiscriminant::Finalize),
            0x3 => Ok(DlpDiscriminant::Undelegate),
            0x4 => Ok(DlpDiscriminant::AllowUndelegate),
            0x5 => Ok(DlpDiscriminant::InitFeesVault),
            0x6 => Ok(DlpDiscriminant::InitValidatorFeesVault),
            0x7 => Ok(DlpDiscriminant::ValidatorClaimFees),
            0x8 => Ok(DlpDiscriminant::WhitelistValidatorForProgram),
            0x9 => Ok(DlpDiscriminant::TopUpEphemeralBalance),
            0xa => Ok(DlpDiscriminant::DelegateEphemeralBalance),
            0xb => Ok(DlpDiscriminant::CloseEphemeralBalance),
            _ => Err(ProgramError::InvalidInstructionData),
        }
    }
}
