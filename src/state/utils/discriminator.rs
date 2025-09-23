use num_enum::{IntoPrimitive, TryFromPrimitive};
use solana_program_error::ProgramError;

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, IntoPrimitive, TryFromPrimitive)]
pub enum AccountDiscriminator {
    DelegationRecord = 100,
    DelegationMetadata = 102,
    CommitRecord = 101,
    ProgramConfig = 103,
    DelegatedCompressedAccount = 104,
}

impl AccountDiscriminator {
    pub const fn to_bytes(&self) -> [u8; 8] {
        let num = (*self) as u64;
        num.to_le_bytes()
    }

    pub const fn try_from_bytes(bytes: [u8; 8]) -> Result<Self, ProgramError> {
        match u64::from_le_bytes(bytes) {
            100 => Ok(Self::DelegationRecord),
            102 => Ok(Self::DelegationMetadata),
            101 => Ok(Self::CommitRecord),
            103 => Ok(Self::ProgramConfig),
            104 => Ok(Self::DelegatedCompressedAccount),
            _ => Err(ProgramError::InvalidAccountData),
        }
    }
}

pub trait AccountWithDiscriminator {
    fn discriminator() -> AccountDiscriminator;
}
