mod commit_diff;
mod commit_diff_from_buffer;
mod commit_finalize;
mod commit_finalize_internal;
mod commit_state;
mod commit_state_from_buffer;
mod delegate;
mod finalize;
mod undelegate;
mod undelegate_confined_account;
mod utils;

pub use commit_diff::*;
pub use commit_diff_from_buffer::*;
pub use commit_finalize::*;
pub use commit_state::*;
pub use commit_state_from_buffer::*;
pub use delegate::*;
pub use finalize::*;
pub use undelegate::*;
pub use undelegate_confined_account::*;

pub fn to_pinocchio_program_error(
    error: solana_program::program_error::ProgramError,
) -> pinocchio::program_error::ProgramError {
    u64::from(error).into()
}
