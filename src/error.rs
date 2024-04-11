use num_enum::IntoPrimitive;
use solana_program::program_error::ProgramError;
use thiserror::Error;

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq, IntoPrimitive)]
#[repr(u32)]
pub enum DlpError {
    #[error("Invalid Authority")]
    InvalidAuthority = 0,
}

impl From<DlpError> for ProgramError {
    fn from(e: DlpError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
