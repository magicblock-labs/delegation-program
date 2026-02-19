#[macro_export]
macro_rules! impl_try_from_bytes_with_discriminator_zero_copy {
    ($struct_name:ident) => {
        impl $struct_name {
            pub fn try_from_bytes_with_discriminator(
                data: &[u8],
            ) -> Result<&Self, ::solana_program::program_error::ProgramError> {
                let expected_len = 8 + ::std::mem::size_of::<Self>();
                if data.len() < expected_len {
                    return Err($crate::error::DlpError::InvalidDataLength.into());
                }
                if Self::discriminator().to_bytes().ne(&data[..8]) {
                    return Err($crate::error::DlpError::InvalidDiscriminator.into());
                }
                bytemuck::try_from_bytes::<Self>(&data[8..expected_len]).or(Err(
                    $crate::error::DlpError::InvalidDelegationRecordData.into(),
                ))
            }
            pub fn try_from_bytes_with_discriminator_mut(
                data: &mut [u8],
            ) -> Result<&mut Self, ::solana_program::program_error::ProgramError> {
                let expected_len = 8 + ::std::mem::size_of::<Self>();
                if data.len() < expected_len {
                    return Err($crate::error::DlpError::InvalidDataLength.into());
                }
                if Self::discriminator().to_bytes().ne(&data[..8]) {
                    return Err($crate::error::DlpError::InvalidDiscriminator.into());
                }
                bytemuck::try_from_bytes_mut::<Self>(&mut data[8..expected_len]).or(Err(
                    $crate::error::DlpError::InvalidDelegationRecordData.into(),
                ))
            }
        }
    };
}

#[macro_export]
macro_rules! impl_try_from_bytes_with_discriminator_borsh {
    ($struct_name:ident) => {
        impl $struct_name {
            pub fn try_from_bytes_with_discriminator(
                data: &[u8],
            ) -> Result<Self, ::solana_program::program_error::ProgramError>
            {
                if data.len() < 8 {
                    return Err(
                        $crate::error::DlpError::InvalidDataLength.into()
                    );
                }
                if Self::discriminator().to_bytes().ne(&data[..8]) {
                    return Err(
                        $crate::error::DlpError::InvalidDiscriminator.into()
                    );
                }
                Self::try_from_slice(&data[8..]).or(Err(
                    $crate::error::DlpError::InvalidDelegationRecordData.into(),
                ))
            }
        }
    };
}
