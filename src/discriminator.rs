use num_enum::TryFromPrimitive;
use solana_program::program_error::ProgramError;

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, TryFromPrimitive)]
#[rustfmt::skip]
pub enum DlpDiscriminator {
    /// Delegates an account
    /// See [crate::processor::process_delegate] for docs.
    Delegate = 0,
    CommitState = 1,
    Finalize = 2,
    Undelegate = 3,
    InitFeesVault = 5,
    InitValidatorFeesVault = 6,
    ValidatorClaimFees = 7,
    WhitelistValidatorForProgram = 8,
    TopUpEphemeralBalance = 9,
    DelegateEphemeralBalance = 10,
    CloseEphemeralBalance = 11,
    ProtocolClaimFees = 12,
    CommitStateFromBuffer = 13,
}

impl DlpDiscriminator {
    pub fn to_vec(self) -> Vec<u8> {
        let num = self as u64;
        num.to_le_bytes().to_vec()
    }
}

impl TryFrom<[u8; 8]> for DlpDiscriminator {
    type Error = ProgramError;
    fn try_from(bytes: [u8; 8]) -> Result<Self, Self::Error> {
        match bytes[0] {
            0x0 => Ok(DlpDiscriminator::Delegate),
            0x1 => Ok(DlpDiscriminator::CommitState),
            0x2 => Ok(DlpDiscriminator::Finalize),
            0x3 => Ok(DlpDiscriminator::Undelegate),
            0x5 => Ok(DlpDiscriminator::InitFeesVault),
            0x6 => Ok(DlpDiscriminator::InitValidatorFeesVault),
            0x7 => Ok(DlpDiscriminator::ValidatorClaimFees),
            0x8 => Ok(DlpDiscriminator::WhitelistValidatorForProgram),
            0x9 => Ok(DlpDiscriminator::TopUpEphemeralBalance),
            0xa => Ok(DlpDiscriminator::DelegateEphemeralBalance),
            0xb => Ok(DlpDiscriminator::CloseEphemeralBalance),
            0xc => Ok(DlpDiscriminator::ProtocolClaimFees),
            0xd => Ok(DlpDiscriminator::CommitStateFromBuffer),
            _ => Err(ProgramError::InvalidInstructionData),
        }
    }
}
