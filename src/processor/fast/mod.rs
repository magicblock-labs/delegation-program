mod commit_state;
mod delegate;
mod undelegate;
mod utils;

pub use commit_state::*;
pub use delegate::*;
pub use undelegate::*;

pub fn to_pinocchio_program_error(
    error: solana_program::program_error::ProgramError,
) -> pinocchio::program_error::ProgramError {
    u64::from(error).into()
}
