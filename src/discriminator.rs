use num_enum::TryFromPrimitive;
use solana_program::program_error::ProgramError;

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, TryFromPrimitive)]
#[rustfmt::skip]
pub enum DlpDiscriminator {
    /// See [crate::processor::process_delegate] for docs.
    Delegate = 0,
    /// See [crate::processor::process_commit_state] for docs.
    CommitState = 1,
    /// See [crate::processor::process_finalize] for docs.
    Finalize = 2,
    /// See [crate::processor::process_undelegate] for docs.
    Undelegate = 3,
    /// See [crate::processor::process_init_protocol_fees_vault] for docs.
    InitProtocolFeesVault = 5,
    /// See [crate::processor::process_init_validator_fees_vault] for docs.
    InitValidatorFeesVault = 6,
    /// See [crate::processor::process_validator_claim_fees] for docs.
    ValidatorClaimFees = 7,
    /// See [crate::processor::process_whitelist_validator_for_program] for docs.
    WhitelistValidatorForProgram = 8,
    /// See [crate::processor::process_top_up_ephemeral_balance] for docs.
    TopUpEphemeralBalance = 9,
    /// See [crate::processor::process_delegate_ephemeral_balance] for docs.
    DelegateEphemeralBalance = 10,
    /// See [crate::processor::process_close_ephemeral_balance] for docs.
    CloseEphemeralBalance = 11,
    /// See [crate::processor::process_protocol_claim_fees] for docs.
    ProtocolClaimFees = 12,
    /// See [crate::processor::process_commit_state_from_buffer] for docs.
    CommitStateFromBuffer = 13,
    /// See [crate::processor::process_close_validator_fees_vault] for docs.
    CloseValidatorFeesVault = 14,
    /// see [crate::processor::process_external_undelegate] for docs.
    ExternalUndelegate = 196,
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
            0x5 => Ok(DlpDiscriminator::InitProtocolFeesVault),
            0x6 => Ok(DlpDiscriminator::InitValidatorFeesVault),
            0x7 => Ok(DlpDiscriminator::ValidatorClaimFees),
            0x8 => Ok(DlpDiscriminator::WhitelistValidatorForProgram),
            0x9 => Ok(DlpDiscriminator::TopUpEphemeralBalance),
            0xa => Ok(DlpDiscriminator::DelegateEphemeralBalance),
            0xb => Ok(DlpDiscriminator::CloseEphemeralBalance),
            0xc => Ok(DlpDiscriminator::ProtocolClaimFees),
            0xd => Ok(DlpDiscriminator::CommitStateFromBuffer),
            0xe => Ok(DlpDiscriminator::CloseValidatorFeesVault),
            196 => Ok(DlpDiscriminator::ExternalUndelegate),
            _ => Err(ProgramError::InvalidInstructionData),
        }
    }
}
