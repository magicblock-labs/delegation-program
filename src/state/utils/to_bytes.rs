#[macro_export]
macro_rules! impl_to_bytes_with_discriminator_zero_copy {
    ($struct_name:ident) => {
        impl $struct_name {
            pub fn to_bytes_with_discriminator(
                &self,
                data: &mut [u8],
            ) -> Result<(), ::solana_program::program_error::ProgramError> {
                if data.len() < 8 {
                    return Err(::solana_program::program_error::ProgramError::InvalidAccountData);
                }
                data[..8].copy_from_slice(&Self::discriminator().to_bytes());
                data[8..].copy_from_slice(bytemuck::bytes_of(self));
                Ok(())
            }
        }
    };
}

#[macro_export]
macro_rules! impl_to_bytes_with_discriminator_borsh {
    ($struct_name:ident) => {
        impl $struct_name {
            pub fn to_bytes_with_discriminator<W: std::io::Write>(
                &self,
                data: &mut W,
            ) -> Result<(), ::solana_program::program_error::ProgramError> {
                data.write_all(&Self::discriminator().to_bytes())?;
                self.serialize(data)?;
                Ok(())
            }
        }
    };
}
