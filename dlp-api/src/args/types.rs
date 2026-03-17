use std::ops::Deref;

use bytemuck::{Pod, Zeroable};
use pinocchio::error::ProgramError;

use crate::{pod_view::PodView, require_ge};

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
pub struct ArgsWithBuffer<'a, Header> {
    header: &'a Header,
    pub buffer: &'a [u8],
}

impl<'a, Header: PodView> ArgsWithBuffer<'a, Header> {
    pub fn from_bytes(input: &'a [u8]) -> Result<Self, ProgramError> {
        require_ge!(
            input.len(),
            Header::SPACE,
            ProgramError::InvalidInstructionData
        );

        let (header_bytes, buffer) = input.split_at(Header::SPACE);

        Ok(Self {
            header: Header::try_view_from(header_bytes)?,
            buffer,
        })
    }
}

impl<H> Deref for ArgsWithBuffer<'_, H> {
    type Target = H;
    fn deref(&self) -> &Self::Target {
        self.header
    }
}
