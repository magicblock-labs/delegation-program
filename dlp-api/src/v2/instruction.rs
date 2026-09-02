use num_enum::TryFromPrimitive;
use strum::IntoStaticStr;

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, TryFromPrimitive, IntoStaticStr)]
#[rustfmt::skip]
pub enum DlpV2Instruction {
    /// Creates the global v2 protocol config and verifier registry accounts.
    InitProtocolConfig = 100,
    /// Registers one operator and deposits its initial stake.
    RegisterOperator = 101,
    /// Registers one verifier and deposits its initial stake.
    RegisterVerifier = 102,
    /// Updates the set of verifiers that can be selected.
    UpdateVerifierRegistry = 103,
    /// Updates global v2 config for future commitments.
    UpdateProtocolConfig = 104,
    /// Writes full account-state bytes into a v2 state buffer.
    ///
    /// TODO (snawaz/optimization): we can split this into two instructions such that 
    /// InitStateBuffer takes more arguments and AppendStateBuffer takes as less as
    /// possible. 
    WriteStateBuffer = 107,
}

impl DlpV2Instruction {
    pub fn to_vec(self) -> Vec<u8> {
        vec![self as u8]
    }

    pub fn name(&self) -> &'static str {
        self.into()
    }
}
