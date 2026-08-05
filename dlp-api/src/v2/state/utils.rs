use wheels::DataLayoutError;

use crate::{error::DlpError, solana_program::program_error::ProgramError};

pub fn layout_error_to_program_error(error: DataLayoutError) -> ProgramError {
    ProgramError::Custom(error.code())
}

pub(crate) fn payload_with_discriminator<'a>(
    discriminator: &[u8; 8],
    data: &'a [u8],
) -> Result<&'a [u8], ProgramError> {
    if data.len() < 8 {
        return Err(DlpError::InvalidDataLength.into());
    }

    if discriminator.as_slice() != &data[..8] {
        return Err(DlpError::InvalidDiscriminator.into());
    }

    Ok(&data[8..])
}

pub(crate) fn payload_with_discriminator_mut<'a>(
    discriminator: &[u8; 8],
    data: &'a mut [u8],
) -> Result<&'a mut [u8], ProgramError> {
    if data.len() < 8 {
        return Err(DlpError::InvalidDataLength.into());
    }

    data[..8].copy_from_slice(discriminator);
    Ok(&mut data[8..])
}
