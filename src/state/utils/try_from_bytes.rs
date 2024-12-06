#[macro_export]
macro_rules! impl_try_from_bytes_with_discriminant_zero_copy {
    ($struct_name:ident) => {
        impl $struct_name {
            pub fn try_from_bytes_with_discriminant(
                data: &[u8],
            ) -> Result<&Self, solana_program::program_error::ProgramError> {
                if Self::discriminant().ne(&data[..8]) {
                    return Err(solana_program::program_error::ProgramError::InvalidAccountData);
                }
                bytemuck::try_from_bytes::<Self>(&data[8..]).or(Err(
                    solana_program::program_error::ProgramError::InvalidAccountData,
                ))
            }
            pub fn try_from_bytes_with_discriminant_mut(
                data: &mut [u8],
            ) -> Result<&mut Self, solana_program::program_error::ProgramError> {
                if Self::discriminant().ne(&data[..8]) {
                    return Err(solana_program::program_error::ProgramError::InvalidAccountData);
                }
                bytemuck::try_from_bytes_mut::<Self>(&mut data[8..]).or(Err(
                    solana_program::program_error::ProgramError::InvalidAccountData,
                ))
            }
        }
    };
}

#[macro_export]
macro_rules! impl_try_from_bytes_with_discriminant_borsh {
    ($struct_name:ident) => {
        impl $struct_name {
            pub fn try_from_bytes_with_discriminant(
                data: &[u8],
            ) -> Result<Self, solana_program::program_error::ProgramError> {
                if Self::discriminant().ne(&data[..8]) {
                    return Err(solana_program::program_error::ProgramError::InvalidAccountData);
                }
                Self::try_from_slice(&data[8..]).or(Err(
                    solana_program::program_error::ProgramError::InvalidAccountData,
                ))
            }
        }
    };
}
