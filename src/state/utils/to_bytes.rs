#[macro_export]
macro_rules! impl_to_bytes_with_discriminant_zero_copy {
    ($struct_name:ident) => {
        impl $struct_name {
            pub fn to_bytes_with_discriminant(
                &self,
                data: &mut [u8],
            ) -> Result<(), solana_program::program_error::ProgramError> {
                data[..8].copy_from_slice(Self::discriminant());
                data[8..].copy_from_slice(bytemuck::bytes_of(self));
                Ok(())
            }
        }
    };
}

#[macro_export]
macro_rules! impl_to_bytes_with_discriminant_borsh {
    ($struct_name:ident) => {
        impl $struct_name {
            pub fn to_bytes_with_discriminant<W: std::io::Write>(
                &self,
                data: &mut W,
            ) -> Result<(), solana_program::program_error::ProgramError> {
                data.write_all(Self::discriminant())?;
                self.serialize(data)?;
                Ok(())
            }
        }
    };
}
