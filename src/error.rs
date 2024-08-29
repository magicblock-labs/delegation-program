use num_enum::IntoPrimitive;
use solana_program::program_error::ProgramError;
use thiserror::Error;

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq, IntoPrimitive)]
#[repr(u32)]
pub enum DlpError {
    #[error("Invalid Authority")]
    InvalidAuthority = 0,
    #[error("Account cannot be undelegated, is_delegatable is false and valid_until isn't reached")]
    Undelegatable = 1,
    #[error("Unathorized Operation")]
    Unauthorized = 2,
    #[error("Invalid Authority for the current target program")]
    InvalidAuthorityForProgram = 3,
    #[error("Delegated account does not match the expected account")]
    InvalidDelegatedAccount = 4,
    #[error("Reimbursement account does not match the expected account")]
    InvalidReimbursementAccount = 5,
}

impl From<DlpError> for ProgramError {
    fn from(e: DlpError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
