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
    /// Posts a new v2 account-state commitment.
    PostCommitment = 105,
    /// Records approval from the selected verifier for a v2 commitment.
    ApproveCommitment = 106,
    /// Writes full account-state bytes into a v2 state buffer.
    ///
    /// TODO (snawaz/optimization): we can split this into two instructions such that 
    /// InitStateBuffer takes more arguments and AppendStateBuffer takes as less as
    /// possible. 
    WriteStateBuffer = 107,
    /// Applies an approved v2 commitment to the delegated account.
    FinalizeCommitment = 108,
    /// Raises a hash-only challenge against a v2 pending commitment.
    RaiseChallenge = 109,
    /// Reveals challenger state for a v2 challenge.
    ChallengerReveal = 110,
}

impl DlpV2Instruction {
    pub fn to_vec(self) -> Vec<u8> {
        vec![self as u8]
    }

    pub fn name(&self) -> &'static str {
        self.into()
    }
}
