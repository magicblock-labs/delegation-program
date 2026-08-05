use crate::{
    compat::borsh::{BorshDeserialize, BorshSerialize},
    error::DlpError,
    solana_program::program_error::ProgramError,
};

pub(crate) fn write_with_discriminator<T, W>(
    discriminator: &[u8; 8],
    value: &T,
    writer: &mut W,
) -> Result<(), ProgramError>
where
    T: BorshSerialize,
    W: std::io::Write,
{
    writer.write_all(discriminator)?;
    value.serialize(writer)?;
    Ok(())
}

pub(crate) fn try_from_bytes_with_discriminator<T>(
    discriminator: &[u8; 8],
    data: &[u8],
) -> Result<T, ProgramError>
where
    T: BorshDeserialize,
{
    if data.len() < 8 {
        return Err(DlpError::InvalidDataLength.into());
    }

    if discriminator.as_slice() != &data[..8] {
        return Err(DlpError::InvalidDiscriminator.into());
    }

    T::try_from_slice(&data[8..])
        .or(Err(DlpError::InvalidDelegationRecordData.into()))
}
