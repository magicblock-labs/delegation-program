#[macro_export]
macro_rules! impl_to_bytes_with_discriminator_zero_copy {
    ($struct_name:ident) => {
        impl $struct_name {
            pub fn to_bytes_with_discriminator(
                &self,
                data: &mut [u8],
            ) -> Result<(), ::solana_program::program_error::ProgramError> {
                let expected_len = 8 + ::std::mem::size_of::<Self>();
                if data.len() != expected_len {
                    return Err(
                        $crate::error::DlpError::InvalidDataLength.into()
                    );
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
                writer: &mut W,
            ) -> Result<(), ::solana_program::program_error::ProgramError> {
                writer.write_all(&Self::discriminator().to_bytes())?;
                self.serialize(writer)?;
                Ok(())
            }
        }
    };
}
