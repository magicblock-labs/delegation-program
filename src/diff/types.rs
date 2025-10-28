use core::{
    mem::{align_of, size_of},
    slice,
};
use std::marker::PhantomData;

use pinocchio::program_error::ProgramError;
use static_assertions::const_assert;

use crate::error::DlpError;

#[derive(Debug, Clone, Copy)]
pub enum SizeChanged {
    Expanded(usize),
    Shrunk(usize),
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct OffsetPair {
    pub offset_in_diff: u32,
    pub offset_in_data: u32,
}

const_assert!(align_of::<OffsetPair>() == align_of::<u32>());
const_assert!(size_of::<OffsetPair>() == 8);

pub struct OffsetInData(pub usize);

pub const SIZE_OF_CHANGED_LEN: usize = size_of::<u32>();
pub const SIZE_OF_NUM_OFFSET_PAIRS: usize = size_of::<u32>();
pub const SIZE_OF_SINGLE_OFFSET_PAIR: usize = size_of::<OffsetPair>();

pub struct DiffSet<'a> {
    buf: *const u8,
    len: usize,
    _marker: PhantomData<&'a [u8]>,
}

impl<'a> DiffSet<'a> {
    pub fn try_new(diff: &'a [u8]) -> Result<Self, ProgramError> {
        if diff.len() < (SIZE_OF_CHANGED_LEN + SIZE_OF_NUM_OFFSET_PAIRS) {
            return Err(DlpError::InvalidDiff.into());
        } else if diff.as_ptr().align_offset(align_of::<u32>()) != 0 {
            return Err(DlpError::InvalidDiffAlignment.into());
        }

        let this = Self {
            buf: diff.as_ptr(),
            len: diff.len(),
            _marker: PhantomData,
        };

        let header_size = SIZE_OF_CHANGED_LEN
            + SIZE_OF_NUM_OFFSET_PAIRS
            + this.num_offset_pairs() * SIZE_OF_SINGLE_OFFSET_PAIR;
        if header_size > diff.len() {
            return Err(DlpError::InvalidDiff.into());
        }

        Ok(this)
    }

    pub fn raw_diff(&self) -> &'a [u8] {
        // SAFETY: it does not do any "computation" as such. It merely reverses try_new
        // and get the immutable slice back.
        unsafe { slice::from_raw_parts(self.buf, self.len) }
    }

    /// Returns the length of the changed data (not diff) that is passed
    /// as the second argument to compute_diff()
    pub fn changed_len(&self) -> usize {
        // SAFETY: try_new enforces the length and alignment requirement:
        // - SIZE_OF_CHANGED_LEN
        // - align_of(u32)
        unsafe { *(self.buf as *const u32) as usize }
    }

    /// Returns the number of offset pairs
    pub fn num_offset_pairs(&self) -> usize {
        // SAFETY: try_new enforces the length and alignment requirement:
        // - SIZE_OF_CHANGED_LEN + SIZE_OF_NUM_OFFSET_PAIRS
        // - align_of(u32)
        unsafe { *(self.buf.add(SIZE_OF_CHANGED_LEN) as *const u32) as usize }
    }

    /// Returns the offset pairs
    pub fn offset_pairs(&self) -> &'a [OffsetPair] {
        // SAFETY: try_new enforces length and alignment:
        // - buf is aligned to 4-byte, so is buf.add(4 + 4).
        //   - both SIZE_OF_CHANGED_LEN and SIZE_OF_NUM_OFFSET_PAIRS are 4.
        // - header_size validation ensures buffer length to &[OffsetPair].
        unsafe {
            let pairs_ptr = self.buf.add(SIZE_OF_CHANGED_LEN + SIZE_OF_NUM_OFFSET_PAIRS);
            slice::from_raw_parts(pairs_ptr as *const OffsetPair, self.num_offset_pairs())
        }
    }

    ///
    /// Returns a diff-slice at the given index and also returns the offset-in-account-data
    /// where the returned diff-slice is to be applied.
    ///
    pub fn diff_slice_at(&self, index: usize) -> Option<(&'a [u8], OffsetInData)> {
        let num_slices = self.num_offset_pairs();
        if index >= num_slices {
            return None;
        }
        let offsets = self.offset_pairs();
        let current_offset = offsets[index];
        let slice_len = {
            if index + 1 < num_slices {
                offsets[index + 1].offset_in_diff - current_offset.offset_in_diff
            } else {
                self.concatenated_diff_slice_len() as u32 - current_offset.offset_in_diff
            }
        };
        Some((
            unsafe {
                slice::from_raw_parts(
                    self.concatenated_diff_slice_begin()
                        .add(current_offset.offset_in_diff as usize),
                    slice_len as usize,
                )
            },
            OffsetInData(current_offset.offset_in_data as usize),
        ))
    }

    /// Returns the address of the beginning of the concatenated-diff.  
    fn concatenated_diff_slice_begin(&self) -> *const u8 {
        unsafe {
            self.buf
                .add(SIZE_OF_CHANGED_LEN + SIZE_OF_NUM_OFFSET_PAIRS)
                .add(self.num_offset_pairs() * SIZE_OF_SINGLE_OFFSET_PAIR)
        }
    }

    /// Returns the length of the concatenated-diff.  
    fn concatenated_diff_slice_len(&self) -> usize {
        self.len
            - (SIZE_OF_CHANGED_LEN
                + SIZE_OF_NUM_OFFSET_PAIRS
                + self.num_offset_pairs() * SIZE_OF_SINGLE_OFFSET_PAIR)
    }
}
