use num_enum::TryFromPrimitive;
use strum::IntoStaticStr;

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, TryFromPrimitive, IntoStaticStr)]
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
    /// See [crate::processor::process_call_handler] for docs.
    CallHandler = 15,
    /// See [crate::processor::process_commit_diff] for docs.
    CommitDiff = 16,
    /// See [crate::processor::process_commit_diff_from_buffer] for docs.
    CommitDiffFromBuffer = 17,
}

impl DlpDiscriminator {
    pub fn to_vec(self) -> Vec<u8> {
        let num = self as u64;
        num.to_le_bytes().to_vec()
    }

    pub fn name(&self) -> &'static str {
        self.into()
    }
}
