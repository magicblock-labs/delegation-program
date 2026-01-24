use std::ops::Deref;

use bytemuck::{Pod, Zeroable};
use pinocchio::program_error::ProgramError;

use crate::pod_view::PodView;

///
/// Boolean
///
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable, Default)]
pub struct Boolean(u8);

impl Boolean {
    pub fn is_true(&self) -> bool {
        // any non-zero is true
        self.0 != 0
    }
    pub fn is_false(&self) -> bool {
        self.0 == 0
    }
}

impl From<bool> for Boolean {
    fn from(value: bool) -> Self {
        Self(if value { 1 } else { 0 })
    }
}

///
/// ArgsWithBuffer
///
pub struct ArgsWithBuffer<'a, H> {
    header: &'a H,
    pub buffer: &'a [u8],
}

impl<'a, H: PodView> ArgsWithBuffer<'a, H> {
    pub fn from_bytes(input: &'a [u8]) -> Result<Self, ProgramError> {
        let header_size = size_of::<H>();

        if input.len() < header_size {
            return Err(ProgramError::InvalidInstructionData);
        }

        let (header_bytes, buffer) = input.split_at(header_size);

        let header = H::try_view_from(header_bytes)?;

        Ok(Self { header, buffer })
    }
}

impl<H> Deref for ArgsWithBuffer<'_, H> {
    type Target = H;
    fn deref(&self) -> &Self::Target {
        self.header
    }
}
