use std::ops::Deref;

use bytemuck::Pod;
use pinocchio::program_error::ProgramError;

pub struct ArgsWithBuffer<'a, H> {
    header: &'a H,
    pub buffer: &'a [u8],
}

impl<'a, H: Pod> ArgsWithBuffer<'a, H> {
    pub fn from_bytes(input: &'a [u8]) -> Result<Self, ProgramError> {
        let header_size = size_of::<H>();

        if input.len() < header_size {
            return Err(ProgramError::InvalidInstructionData);
        }

        let (header_bytes, buffer) = input.split_at(header_size);
        let header = bytemuck::from_bytes::<H>(header_bytes);

        Ok(Self { header, buffer })
    }
}

impl<'a, H> Deref for ArgsWithBuffer<'a, H> {
    type Target = H;
    fn deref(&self) -> &Self::Target {
        self.header
    }
}
