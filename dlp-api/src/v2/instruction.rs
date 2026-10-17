use num_enum::TryFromPrimitive;
use strum::IntoStaticStr;

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, TryFromPrimitive, IntoStaticStr)]
#[rustfmt::skip]
pub enum DlpV2Instruction {
    /// Creates the global v2 protocol config and verifier registry accounts.
    InitProtocolConfig = 100,
}

impl DlpV2Instruction {
    pub fn to_vec(self) -> Vec<u8> {
        vec![self as u8]
    }

    pub fn name(&self) -> &'static str {
        self.into()
    }
}
