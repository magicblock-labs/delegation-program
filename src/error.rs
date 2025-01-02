use num_enum::IntoPrimitive;
use solana_program::program_error::ProgramError;
use thiserror::Error;

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq, IntoPrimitive)]
#[repr(u32)]
pub enum DlpError {
    #[error("Invalid Authority")]
    InvalidAuthority = 0,
    #[error("Account cannot be undelegated, is_delegatable is false")]
    Undelegatable = 1,
    #[error("Unauthorized Operation")]
    Unauthorized = 2,
    #[error("Invalid Authority for the current target program")]
    InvalidAuthorityForProgram = 3,
    #[error("Delegated account does not match the expected account")]
    InvalidDelegatedAccount = 4,
    #[error("Delegated account is not in a valid state")]
    InvalidDelegatedState = 5,
    #[error("Reimbursement account does not match the expected account")]
    InvalidReimbursementAccount = 6,
    #[error("Invalid account data after CPI")]
    InvalidAccountDataAfterCPI = 7,
    #[error("Invalid validator balance after CPI")]
    InvalidValidatorBalanceAfterCPI = 8,
    #[error("Invalid reimbursement address for delegation rent")]
    InvalidReimbursementAddressForDelegationRent = 9,
    #[error("Authority is invalid for the delegated account program owner")]
    InvalidWhitelistProgramConfig = 10,
    #[error("Account already undelegated")]
    AlreadyUndelegated = 11,
    #[error("Committed state slot is outdated")]
    OutdatedSlot = 12,
    #[error("Computation overflow detected")]
    Overflow = 13,
}

impl From<DlpError> for ProgramError {
    fn from(e: DlpError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
