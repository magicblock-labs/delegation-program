mod delegate;
mod utils;

pub use delegate::*;

pub fn to_pinocchio_program_error(
    error: solana_program::program_error::ProgramError,
) -> pinocchio::program_error::ProgramError {
    u64::from(error).into()
}
