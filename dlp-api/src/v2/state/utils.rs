use wheels::DataLayoutError;

use crate::solana_program::program_error::ProgramError;

pub fn layout_error_to_program_error(error: DataLayoutError) -> ProgramError {
    ProgramError::Custom(error.code())
}
