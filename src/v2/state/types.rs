use std::ops::{Deref, DerefMut};

use pinocchio::error::ProgramError;

use crate::{pod_view::PodView, v2_require_ge};

pub struct HeaderWithBuffer<'a, Header> {
    header: &'a Header,
    pub buffer: &'a [u8],
}

impl<'a, Header: PodView> HeaderWithBuffer<'a, Header> {
    #[inline(always)]
    pub fn from_bytes(input: &'a [u8]) -> Result<Self, ProgramError> {
        v2_require_ge!(
            input.len(),
            Header::SPACE,
            ProgramError::InvalidInstructionData
        );

        let (header_bytes, buffer) =
            unsafe { input.split_at_unchecked(Header::SPACE) };

        Ok(Self {
            header: {
                #[cfg(feature = "unsafe")]
                unsafe {
                    &*(header_bytes.as_ptr() as *const Header)
                }

                #[cfg(not(feature = "unsafe"))]
                Header::try_view_from(header_bytes)?
            },
            buffer,
        })
    }

    #[inline(always)]
    pub fn from_bytes_ptr(
        data: *const u8,
        len: usize,
    ) -> Result<Self, ProgramError> {
        v2_require_ge!(
            len,
            Header::SPACE,
            ProgramError::InvalidInstructionData
        );

        Ok(Self {
            header: {
                //#[cfg(feature = "unsafe")]
                unsafe { &*(data as *const Header) }

                //#[cfg(not(feature = "unsafe"))]
                //Header::try_view_from(header_bytes)?
            },
            buffer: unsafe {
                core::slice::from_raw_parts(
                    data.add(Header::SPACE),
                    len - Header::SPACE,
                )
            },
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

pub struct HeaderWithBufferMut<'a, Header> {
    header: &'a mut Header,
    pub buffer: &'a mut [u8],
}

impl<'a, Header: PodView> HeaderWithBufferMut<'a, Header> {
    #[inline(always)]
    pub fn from_bytes(input: &'a mut [u8]) -> Result<Self, ProgramError> {
        v2_require_ge!(
            input.len(),
            Header::SPACE,
            ProgramError::InvalidInstructionData
        );

        let (header_bytes, buffer) =
            unsafe { input.split_at_mut_unchecked(Header::SPACE) };

        Ok(Self {
            header: {
                #[cfg(feature = "unsafe")]
                unsafe {
                    &mut *(header_bytes.as_mut_ptr() as *mut Header)
                }

                #[cfg(not(feature = "unsafe"))]
                Header::try_view_from_mut(header_bytes)?
            },
            buffer,
        })
    }

    pub fn space(&self) -> usize {
        Header::SPACE + self.buffer.len()
    }
}

impl<Header> Deref for HeaderWithBufferMut<'_, Header> {
    type Target = Header;
    fn deref(&self) -> &Self::Target {
        self.header
    }
}

impl<Header> DerefMut for HeaderWithBufferMut<'_, Header> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.header
    }
}
