use core::{
    mem::{align_of, size_of},
    slice,
};
use std::{cmp::Ordering, ops::Range};

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

// half-open semantic: [start, end)
pub type OffsetInData = Range<usize>;

pub const SIZE_OF_CHANGED_LEN: usize = size_of::<u32>();
pub const SIZE_OF_NUM_OFFSET_PAIRS: usize = size_of::<u32>();
pub const SIZE_OF_SINGLE_OFFSET_PAIR: usize = size_of::<OffsetPair>();

pub struct DiffSet<'a> {
    buf: *const u8,
    buflen: usize,
    changed_len: usize,
    segments_count: usize,
    offset_pairs: &'a [OffsetPair],
    concat_diff: &'a [u8],
}

impl<'a> DiffSet<'a> {
    pub fn try_new(diff: &'a [u8]) -> Result<Self, ProgramError> {
        // Format:
        // | ChangeLen | # Offset Pairs  | Offset Pair 0 | Offset Pair 1 | ... | Concat Diff |
        // |= 4 bytes =|==== 4 bytes ====|=== 8 bytes ===|=== 8 bytes ===| ... |== M bytes ==|

        if diff.len() < (SIZE_OF_CHANGED_LEN + SIZE_OF_NUM_OFFSET_PAIRS) {
            return Err(DlpError::InvalidDiff.into());
        } else if diff.as_ptr().align_offset(align_of::<u32>()) != 0 {
            return Err(DlpError::InvalidDiffAlignment.into());
        }

        // SAFETY: if we are here, that means, diff is good and headers exist:
        //  - buf aligned to 4-byte
        //  - buf is big enough to hold both changed_len and segments_count
        let buf = diff.as_ptr();
        let buflen = diff.len();
        let changed_len = unsafe { *(buf as *const u32) as usize };
        let segments_count = unsafe { *(buf.add(4) as *const u32) as usize };

        let mut this = Self {
            buf,
            buflen,
            changed_len,
            segments_count,
            offset_pairs: &[],
            concat_diff: b"",
        };

        let header_len = SIZE_OF_CHANGED_LEN
            + SIZE_OF_NUM_OFFSET_PAIRS
            + segments_count * SIZE_OF_SINGLE_OFFSET_PAIR;

        match diff.len().cmp(&header_len) {
            Ordering::Equal => {
                // it means diff contains the header only. and concat_diff is actually empty.
                // nothing to do in this case, except the following validation check.
                if this.segments_count() != 0 {
                    return Err(DlpError::InvalidDiff.into());
                }
            }
            Ordering::Less => {
                // diff cannot be smaller than the header size.
                return Err(DlpError::InvalidDiff.into());
            }
            Ordering::Greater => {
                // SAFETY: agaim, if we are here, that means, all invariants are good and now
                // we can read the offset pairs.
                //  - raw_pairs aligned to 4-byte
                //  - raw_pairs is big enough to hold both changed_len and segments_count
                this.offset_pairs = unsafe {
                    let raw_pairs = buf.add(SIZE_OF_CHANGED_LEN + SIZE_OF_NUM_OFFSET_PAIRS)
                        as *const OffsetPair;
                    slice::from_raw_parts(raw_pairs, segments_count)
                };
                this.concat_diff = &diff[header_len..];
            }
        }

        Ok(this)
    }

    pub fn raw_diff(&self) -> &'a [u8] {
        // SAFETY: it does not do any "computation" as such. It merely reverses try_new
        // and get the immutable slice back.
        unsafe { slice::from_raw_parts(self.buf, self.buflen) }
    }

    /// Returns the length of the changed data (not diff) that is passed
    /// as the second argument to compute_diff()
    pub fn changed_len(&self) -> usize {
        self.changed_len
    }

    /// Returns the number of segments which is same as number of offset pairs.
    pub fn segments_count(&self) -> usize {
        self.segments_count
    }

    /// Returns the offset pairs
    pub fn offset_pairs(&self) -> &'a [OffsetPair] {
        self.offset_pairs
    }

    ///
    /// Given an index, returns a diff-segment and offset-range [start,end)
    /// where the returned diff-segment is to be applied in the original data.
    ///
    pub fn diff_segment_at(
        &self,
        index: usize,
    ) -> Result<Option<(&'a [u8], OffsetInData)>, ProgramError> {
        let offsets = self.offset_pairs();
        if index >= offsets.len() {
            return Ok(None);
        }

        let OffsetPair {
            offset_in_diff: segment_begin,
            offset_in_data,
        } = offsets[index];

        let segment_end = if index + 1 < offsets.len() {
            offsets[index + 1].offset_in_diff
        } else {
            self.concat_diff.len() as u32
        };

        // Note: segment is the half-open interval [segment_begin, segment_end)
        if segment_end > self.concat_diff.len() as u32
            || segment_begin >= segment_end
            || offset_in_data >= self.changed_len() as u32
        {
            return Err(DlpError::InvalidDiff.into());
        }

        let segment = &self.concat_diff[segment_begin as usize..segment_end as usize];
        let range =
            offset_in_data as usize..(offset_in_data + segment_end - segment_begin) as usize;

        if range.end > self.changed_len() {
            return Err(DlpError::InvalidDiff.into());
        }

        Ok(Some((segment, range)))
    }

    /// Iterates diff segments
    pub fn iter(
        &self,
    ) -> impl Iterator<Item = Result<(&'a [u8], OffsetInData), ProgramError>> + '_ {
        (0..self.segments_count).map(|index| {
            self.diff_segment_at(index)
                .map(|val| val.expect("impossible: index can never be greater than segments_count"))
        })
    }
}
