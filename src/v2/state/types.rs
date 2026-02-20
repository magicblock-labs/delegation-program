use std::ops::Deref;

use pinocchio::error::ProgramError;

use crate::{pod_view::PodView, v2_require_ge};

pub struct HeaderWithBuffer<'a, Header> {
    header: &'a Header,
    pub buffer: &'a [u8],
}

impl<'a, Header: PodView> HeaderWithBuffer<'a, Header> {
    pub fn from_bytes(input: &'a [u8]) -> Result<Self, ProgramError> {
        v2_require_ge!(
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

    pub fn space(&self) -> usize {
        Header::SPACE + self.buffer.len()
    }
}

impl<Header> Deref for HeaderWithBuffer<'_, Header> {
    type Target = Header;
    fn deref(&self) -> &Self::Target {
        self.header
    }
}
