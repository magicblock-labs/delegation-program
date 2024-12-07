use num_enum::{IntoPrimitive, TryFromPrimitive};

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, IntoPrimitive, TryFromPrimitive)]
pub enum AccountDiscriminator {
    DelegationRecord = 100,
    DelegationMetadata = 102,
    CommitRecord = 101,
    ProgramConfig = 103,
}

impl AccountDiscriminator {
    pub fn to_bytes(&self) -> [u8; 8] {
        let num = (*self) as u64;
        num.to_le_bytes()
    }
}
