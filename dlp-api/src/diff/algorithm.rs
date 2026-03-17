use std::cmp::{min, Ordering};

use pinocchio::error::ProgramError;
use rkyv::util::AlignedVec;

use super::{
    DiffSet, OffsetInData, SizeChanged, SIZE_OF_CHANGED_LEN,
    SIZE_OF_NUM_OFFSET_PAIRS, SIZE_OF_SINGLE_OFFSET_PAIR,
};
use crate::{error::DlpError, require_eq, require_le};

///
/// Compute diff between original and changed.
///
/// Returns:
///
/// - AlignedVec (a vector of u8) which is aligned to 16-byte boundary that can be used
///   to construct DiffSet.
///
/// Note that DiffSet::try_new() requires the argument &[u8] to be aligned to 4-byte
/// boundary, but compute_diff provides much stricter alignment guarantee.
///
/// ref: https://docs.rs/rkyv/0.7.45/rkyv/util/struct.AlignedVec.html
///
/// ---
///
/// Scenarios:
/// - original.len() == changed.len()
///     - diff is computed by comparing corresponding indices.
///     - bytes comparison
///         original: o1 o2 o3 o4 ... oN  
///          changed: c1 c2 c3 c4 ... cN
///     - diff consists of the bytes from the "changed" slice.
/// - original.len() < changed.len()
///     - that implies the account has been reallocated and expanded
///     - bytes comparison
///         original: o1 o2 o3 o4 ... oN  
///          changed: c1 c2 c3 c4 ... cN cN+1 cN+2 ... cN+M
/// - original.len() > changed.len()
///     - that implies the account has been reallocated and shrunk
///     - bytes comparison
///         original: o1 o2 o3 o4 ... oN oN+1 oN+2 ... oN+M
///          changed: c1 c2 c3 c4 ... cN
///    
/// ---
///
/// Diff Format:
///
/// Since an account could have modifications at multiple places (i.e imagine modifying
/// multiple fields in a struct), the diff could consist of multiple segments of [u8], one
/// segment for each contiguous change. So we compute these segments of change, and concatenate them
/// to form one big segment/slice. However, in doing so, we lose the lengths of each segment and the
/// position of change in the original data. So the headers below captures all these necessary information:
///
///  - Length of the given changed data (the second argument to compute_diff).
///  - Number of slices: first 4 bytes.  
///  - Then follows offset-pairs, for each slice/segment, that captures offset in concatenated diff
///    as well as in the original account.
///  - Then follows the concatenated diff bytes.
///
/// |  Length   | # Offset Pairs  | Offset Pair 0 | Offset Pair 1 | ... | Offset Pair N-1 | Concatenated Diff |
/// |= 4 bytes =|==== 4 bytes ====|=== 8 bytes ===|=== 8 bytes ===| ... |==== 8 bytes ====|====  M bytes =====|
///
/// Offset Pair Format:
///
/// | OffsetInDiffBytes  | OffsetInAccountData |
/// | ==== 4 bytes ===== | ===== 4 bytes ===== |
///
/// where,
///
/// - OffsetInDiffBytes is the index in ConcatenatedDiff indicating the beginning of the slice
///   and the next OffsetInDiffBytes from the next pair indicates the end of the slice, i.e the length of
///   slice[i] is OffsetInDiffBytes[i+1] - OffsetInDiffBytes[i].  
///   - Note that the OffsetInDiffBytes is relative to the beginning of ConcatenatedDiff, not
///     relative to the beginning of the whole serialized data, that implies the OffsetInDiffBytes
///     in the first pair is always 0.
/// - M is a variable and is the sum of the length of the diff-slices.
///  - M = diff_segments.map(|s| s.len()).sum()
/// - The length of ith slice = OffsetInDiffBytes@(i+1) - OffsetInDiffBytes@i, as noted earlier.
///
/// ---
///
/// Example,
///
/// Suppose we have an account with datalen = 100:
///
/// ```text
///      ----------------------------
///      |   a 100 bytes account    |
///      ----------------------------
///      | M | A | G | I | ... |  C |
///      ----------------------------
///      | 0 | 1 | 2 | 3 | ... | 99 |
///      ----------------------------
/// ```
///
/// Also, suppose we have modifications at two locations (say u32 and u64), so we have 2 slices of size 4
/// and 8 bytes respectively, and the corresponding indices are the followings (note that these are "indices"
/// in the original account, not the indices in the concatenated diff):
///
///  - | 11 | 12 | 13 | 14 |                     -- 4 bytes
///  - | 71 | 72 | 73 | 74 | 75 | 76 | 77 | 78 | -- 8 bytes
///
/// In this case, the serialized bytes would be:
///
/// ```text
///  ------------------------------------------------------
///  | len | num |   offsets   |  concatenated slices     |
///  ------------------------------------------------------
///  | 100 |  2  | 0 11 | 4 71 | 11 12 13 14 71 72 ... 78 | OffsetInAccountData
///  ------------------------------------------------------
///                            |  0  1  2  3  4  5 ... 11 | OffsetInDiffBytes
///                            ----------------------------
/// ```
///
///  - 100       : u32      : changed.len()
///  - 2         : u32      : number of offset pairs
///  - 0 11      : u32, u32 : first offset pair (offset in diff, offset in account)
///  - 4 71      : u32, u32 : second offset pair (offset in diff, offset in account)
///  - 11 ... 78 : each u8  : concatenated diff bytes
pub fn compute_diff(original: &[u8], changed: &[u8]) -> AlignedVec {
    // 1. identify contiguous diff slices
    let mut diffs: Vec<(usize, &[u8])> = Vec::new();
    let min_len = min(original.len(), changed.len());
    let mut diff_size = 0;
    let mut i = 0;
    while i < min_len {
        if original[i] != changed[i] {
            // start of diff
            let start = i;
            while i < min_len && original[i] != changed[i] {
                i += 1;
            }
            diffs.push((start, &changed[start..i]));
            diff_size += i - start;
        } else {
            i += 1;
        }
    }

    // 2. handle expansion/shrinkage
    match changed.len().cmp(&original.len()) {
        Ordering::Greater => {
            // extra bytes at the end
            diffs.push((original.len(), &changed[original.len()..]));
            diff_size += changed.len() - original.len();
        }
        Ordering::Less => {
            // account shrunk: data truncated
            // nothing to do here
        }
        Ordering::Equal => {
            // already handled
        }
    };

    // 3. serialize according to the spec
    let mut output = AlignedVec::with_capacity(
        SIZE_OF_CHANGED_LEN
            + SIZE_OF_NUM_OFFSET_PAIRS
            + SIZE_OF_SINGLE_OFFSET_PAIR * diffs.len()
            + diff_size,
    );

    // size of changed data (4 bytes)
    output.extend_from_slice(&(changed.len() as u32).to_le_bytes());

    // number of slices / offset-pairs (4 bytes)
    output.extend_from_slice(&(diffs.len() as u32).to_le_bytes());

    // compute offset pairs (offset_in_diff: 4 bytes, offset_in_account: 4 bytes)
    let mut offset_in_diff = 0u32;
    for (offset_in_account, slice) in &diffs {
        output.extend_from_slice(&offset_in_diff.to_le_bytes());
        output.extend_from_slice(&(*offset_in_account as u32).to_le_bytes());
        offset_in_diff += slice.len() as u32;
    }

    // append concatenated diff bytes: concatenated len = sum of len of the diff-segments
    for (_, slice) in diffs {
        output.extend_from_slice(slice);
    }

    output
}

/// Detects if there is size change in the changed data.
///  - None               means NO change
///  - Some(size_changed) means the data size has changed and size_changed indicates
///                       whether it has expanded or shrunk.
pub fn detect_size_change(
    original: &[u8],
    diffset: &DiffSet<'_>,
) -> Option<SizeChanged> {
    match diffset.changed_len().cmp(&original.len()) {
        Ordering::Less => Some(SizeChanged::Shrunk(diffset.changed_len())),
        Ordering::Greater => Some(SizeChanged::Expanded(diffset.changed_len())),
        Ordering::Equal => None,
    }
}

/// This function applies the diff to the first argument (i.e original) to update it.
///
/// Precondition:
///   - original.len() must be equal to the length encoded in the diff.
pub fn apply_diff_in_place(
    original: &mut [u8],
    diffset: &DiffSet<'_>,
) -> Result<(), ProgramError> {
    if let Some(_layout) = detect_size_change(original, diffset) {
        return Err(ProgramError::InvalidInstructionData);
    }
    apply_diff_impl(original, diffset)
}

/// This function creates a copy of original, possibly extending or shrinking it,
/// and then applies the diff to it, before returning it.
pub fn apply_diff_copy(
    original: &[u8],
    diffset: &DiffSet<'_>,
) -> Result<Vec<u8>, ProgramError> {
    Ok(match detect_size_change(original, diffset) {
        Some(SizeChanged::Expanded(new_size)) => {
            let mut applied = Vec::with_capacity(new_size);
            applied.extend_from_slice(original);
            applied.resize(new_size, 0);
            apply_diff_impl(applied.as_mut(), diffset)?;
            applied
        }
        Some(SizeChanged::Shrunk(new_size)) => {
            let mut applied = Vec::from(&original[0..new_size]);
            apply_diff_impl(applied.as_mut(), diffset)?;
            applied
        }
        None => {
            let mut applied = Vec::from(original);
            apply_diff_impl(applied.as_mut(), diffset)?;
            applied
        }
    })
}

/// Constructs destination by applying the diff to original, such that destination becomes the
/// post-diff state of the original.
///
/// Precondition:
///     - destination.len() == diffset.changed_len()
///     - original.len() may differ from destination.len() to allow Solana
///       account resizing (shrink or expand).
/// Assumption:
///     - destination is assumed to be zero-initialized. That automatically holds true for freshly
///       allocated Solana account data. The function does NOT validate this assumption for performance reason.
/// Returns:
///     - Ok(&mut [u8]) where the slice contains the trailing unwritten bytes in destination and are
///       assumed to be already zero-initialized. Callers may write to those bytes or validate it.
/// Notes:
///     - Merge consists of:
///         - bytes covered by diff segments are written from diffset.
///         - unmodified regions are copied directly from original.
///     - In shrink case, extra trailing bytes from original are ignored.
///     - In expansion case, any remaining bytes beyond both the diff coverage
///       and original.len() stay unwritten and are assumed to be zero-initialized.
///
pub fn merge_diff_copy<'a>(
    destination: &'a mut [u8],
    original: &[u8],
    diffset: &DiffSet<'_>,
) -> Result<&'a mut [u8], ProgramError> {
    require_eq!(
        destination.len(),
        diffset.changed_len(),
        DlpError::MergeDiffError
    );

    let mut write_index = 0;
    for item in diffset.iter() {
        let (diff_segment, OffsetInData { start, end }) = item?;

        if write_index < start {
            require_le!(start, original.len(), DlpError::InvalidDiff);
            // copy the unchanged bytes
            destination[write_index..start]
                .copy_from_slice(&original[write_index..start]);
        }

        destination[start..end].copy_from_slice(diff_segment);
        write_index = end;
    }

    // Ensure we have overwritten all bytes in destination, otherwise "construction" of destination
    // will be considered incomplete.
    let num_bytes_written = match write_index.cmp(&destination.len()) {
        Ordering::Equal => {
            // It means the destination is fully constructed.
            // Nothing to do here.

            // It is possible that destination.len() <= original.len() i.e destination might have shrunk
            // in which case we do not care about those bytes of original which are not part of
            // destination anymore.
            write_index
        }
        Ordering::Less => {
            // destination is NOT fully constructed yet. Few bytes in the destination are still unwritten.
            // Let's say the number of these unwritten bytes is: N.
            //
            // Now how do we construct these N unwritten bytes? We have already processed the
            // diffset, so now where could the values for these N bytes come from?
            //
            // There are 3 scenarios:
            // - All N bytes must be copied from remaining region of the original:
            //   - that means, destination.len() <= original.len()
            //   - and the destination might have shrunk, in which case we do not care about
            //     the extra bytes in the original: they're discarded.
            // - Only (N-M) bytes come from original and the rest M bytes stay unwritten and are
            //   "assumed" to be already zero-initialized.
            //   - that means, destination.len() > original.len()
            //   - write_index + (N-M) == original.len()
            //   - and the destination has expanded.
            // - None of these N bytes come from original. It's basically a special case of
            //   the second scenario: when M = N i.e all N bytes stay unwritten.
            //   - that means, destination.len() > original.len()
            //     - and also, write_index == original.len().
            //   - the destination has expanded just like the above case.
            //   - all N bytes are "assumed" to be already zero-initialized (by the caller)

            if destination.len() <= original.len() {
                // case: all n bytes come from original
                let dest_len = destination.len();
                destination[write_index..]
                    .copy_from_slice(&original[write_index..dest_len]);
                dest_len
            } else if write_index < original.len() {
                // case: some bytes come from original and the rest are "assumed" to be
                // zero-initialized (by the caller).
                destination[write_index..original.len()]
                    .copy_from_slice(&original[write_index..]);
                original.len()
            } else {
                // case: all N bytes are "assumed" to be zero-initialized (by the caller).
                write_index
            }
        }
        Ordering::Greater => {
            // It is an impossible scenario. Even if the diff is corrupt, or the lengths of destinatiare are same
            // or different, we'll not encounter this case. It only implies logic error.
            return Err(DlpError::InfallibleError.into());
        }
    };

    let (_, unwritten_bytes) = destination.split_at_mut(num_bytes_written);

    Ok(unwritten_bytes)
}

// private function that does the actual work.
fn apply_diff_impl(
    original: &mut [u8],
    diffset: &DiffSet<'_>,
) -> Result<(), ProgramError> {
    for item in diffset.iter() {
        let (diff_segment, offset_range) = item?;
        original[offset_range].copy_from_slice(diff_segment);
    }
    Ok(())
}

#[cfg(test)]
mod tests {

    use rand::{
        rngs::{OsRng, StdRng},
        seq::IteratorRandom,
        Rng, RngCore, SeedableRng,
    };

    use crate::diff::{
        apply_diff_copy, apply_diff_in_place, compute_diff, merge_diff_copy,
        DiffSet,
    };

    #[test]
    fn test_no_change() {
        let original = [0; 100];
        let diff = compute_diff(&original, &original);

        assert_eq!(diff.len(), 8);
        assert_eq!(
            diff.as_slice(),
            [
                100u32.to_le_bytes().as_slice(),
                0u32.to_le_bytes().as_slice()
            ]
            .concat::<u8>()
        );
    }

    fn get_example_expected_diff(
        changed_len: usize,
        // additional_changes must apply after index 78 (index-in-data) !!
        additional_changes: Vec<(u32, &[u8])>,
    ) -> Vec<u8> {
        // expected: | 100 | 2 | 0 11 | 4 71 | 11 12 13 14 71 72 ... 78 |

        let mut expected_diff = vec![];

        // changed_len (u32)
        expected_diff.extend_from_slice(&(changed_len as u32).to_le_bytes());

        if additional_changes.is_empty() {
            // 2 (u32)
            expected_diff.extend_from_slice(&2u32.to_le_bytes());
        } else {
            expected_diff.extend_from_slice(
                &(2u32 + additional_changes.len() as u32).to_le_bytes(),
            );
        }

        // -- offsets

        // 0 11 (each u32)
        expected_diff.extend_from_slice(&0u32.to_le_bytes());
        expected_diff.extend_from_slice(&11u32.to_le_bytes());

        // 4 71 (each u32)
        expected_diff.extend_from_slice(&4u32.to_le_bytes());
        expected_diff.extend_from_slice(&71u32.to_le_bytes());

        let mut offset_in_diff = 12u32;
        for (offset_in_data, diff) in additional_changes.iter() {
            expected_diff.extend_from_slice(&offset_in_diff.to_le_bytes());
            expected_diff.extend_from_slice(&offset_in_data.to_le_bytes());
            offset_in_diff += diff.len() as u32;
        }

        // -- segments --

        // 11 12 13 14  (each u8)
        expected_diff.extend_from_slice(&0x01020304u32.to_le_bytes());
        // 71 72 ... 78 (each u8)
        expected_diff.extend_from_slice(&0x0102030405060708u64.to_le_bytes());

        // append diff from additional_changes
        for (_, diff) in additional_changes.iter() {
            expected_diff.extend_from_slice(diff);
        }

        expected_diff
    }

    #[test]
    fn test_using_example_data() {
        let original = [0; 100];
        let changed = {
            let mut copy = original;
            // | 11 | 12 | 13 | 14 |
            copy[11..=14].copy_from_slice(&0x01020304u32.to_le_bytes());
            //  | 71 | 72 | 73 | 74 | 75 | 76 | 77 | 78 |
            copy[71..=78].copy_from_slice(&0x0102030405060708u64.to_le_bytes());
            copy
        };

        let actual_diff = compute_diff(&original, &changed);
        let actual_diffset = DiffSet::try_new(&actual_diff).unwrap();
        let expected_diff = get_example_expected_diff(changed.len(), vec![]);

        assert_eq!(actual_diff.len(), 4 + 4 + 8 + 8 + (4 + 8));
        assert_eq!(actual_diff.as_slice(), expected_diff.as_slice());

        let expected_changed =
            apply_diff_copy(&original, &actual_diffset).unwrap();

        assert_eq!(changed.as_slice(), expected_changed.as_slice());

        let expected_changed = {
            let mut destination = vec![255; original.len()];
            merge_diff_copy(&mut destination, &original, &actual_diffset)
                .unwrap();
            destination
        };

        assert_eq!(changed.as_slice(), expected_changed.as_slice());
    }

    #[test]
    fn test_shrunk_account_data() {
        // Note that changed_len cannot be lower than 79 because the last "changed" index is
        // 78 in the diff.
        const CHANGED_LEN: usize = 80;

        let original = vec![0; 100];
        let changed = {
            let mut copy = original.clone();
            copy.truncate(CHANGED_LEN);

            // | 11 | 12 | 13 | 14 |
            copy[11..=14].copy_from_slice(&0x01020304u32.to_le_bytes());
            //  | 71 | 72 | 73 | 74 | 75 | 76 | 77 | 78 |
            copy[71..=78].copy_from_slice(&0x0102030405060708u64.to_le_bytes());
            copy
        };

        let actual_diff = compute_diff(&original, &changed);

        let actual_diffset = DiffSet::try_new(&actual_diff).unwrap();

        let expected_diff = get_example_expected_diff(CHANGED_LEN, vec![]);

        assert_eq!(actual_diff.len(), 4 + 4 + 8 + 8 + (4 + 8));
        assert_eq!(actual_diff.as_slice(), expected_diff.as_slice());

        let expected_changed = {
            let mut destination = vec![255; CHANGED_LEN];
            merge_diff_copy(&mut destination, &original, &actual_diffset)
                .unwrap();
            destination
        };

        assert_eq!(changed.as_slice(), expected_changed.as_slice());
    }

    #[test]
    fn test_expanded_account_data() {
        const CHANGED_LEN: usize = 120;

        let original = vec![0; 100];
        let changed = {
            let mut copy = original.clone();
            copy.resize(CHANGED_LEN, 0); // new bytes are zero-initialized

            // | 11 | 12 | 13 | 14 |
            copy[11..=14].copy_from_slice(&0x01020304u32.to_le_bytes());
            //  | 71 | 72 | 73 | 74 | 75 | 76 | 77 | 78 |
            copy[71..=78].copy_from_slice(&0x0102030405060708u64.to_le_bytes());
            copy
        };

        let actual_diff = compute_diff(&original, &changed);

        let actual_diffset = DiffSet::try_new(&actual_diff).unwrap();

        // When an account expands, the extra bytes at the end become part of the diff, even if
        // all of them are zeroes, that is why (100, &[0; 32]) is passed as additional_changes to
        // the following function.
        //
        // TODO (snawaz): we could optimize compute_diff to not include the zero bytes which are
        // part of the expansion.
        let expected_diff =
            get_example_expected_diff(CHANGED_LEN, vec![(100, &[0; 20])]);

        assert_eq!(actual_diff.len(), 4 + 4 + (8 + 8) + (4 + 8) + (4 + 4 + 20));
        assert_eq!(actual_diff.as_slice(), expected_diff.as_slice());

        let expected_changed = {
            let mut destination = vec![255; CHANGED_LEN];
            let unwritten =
                merge_diff_copy(&mut destination, &original, &actual_diffset)
                    .unwrap();

            // TODO (snawaz): unwritten == &mut [], is because currently the expanded bytes are part of the diff.
            // Once compute_diff is optimized further, written must be &mut [0; 20].
            assert_eq!(unwritten, &mut [] as &mut [u8]);

            destination
        };

        assert_eq!(changed.as_slice(), expected_changed.as_slice());
    }

    #[test]
    fn test_using_large_random_data() {
        // Test Plan:
        // - Use a random account size (between 2 MB and 10 MB).
        // - Mutate it at random, non-overlapping and 8-byte-separated regions.
        // - Use random patch sizes (2–256 bytes).
        // - Verify that apply_diff(original, diff) reproduces changed.

        const MB: usize = 1024 * 1024;

        let seed = OsRng.next_u64();
        println!("Use seed = {seed} to reproduce the input data in case of test failure");

        let mut rng = StdRng::seed_from_u64(seed);

        let original = {
            let account_size: usize = rng.gen_range(2 * MB..10 * MB);
            // println!("account_size: {account_size}");
            let mut data = vec![0u8; account_size];
            rng.fill(&mut data[..]);
            data
        };

        let (changed, slices) = {
            let mut copy = original.clone();
            let mut slices = vec![];

            let slab = MB / 2;
            let mut offset_range = 0..slab;

            while offset_range.end < copy.len() {
                let diff_offset = rng.gen_range(offset_range);
                let diff_len = (1..256).choose(&mut rng).unwrap();
                let diff_end = (diff_offset + diff_len).min(copy.len());

                // Overwrite with new random data
                for i in diff_offset..diff_end {
                    let old = copy[i];
                    while old == copy[i] {
                        copy[i] = rng.gen::<u8>();
                    }
                }
                // println!("{diff_offset}, {diff_end} => {diff_len}");
                slices
                    .push((diff_offset, copy[diff_offset..diff_end].to_vec()));

                offset_range = (diff_end + 8)..(diff_end + 8 + slab);
            }
            (copy, slices)
        };

        let actual_diff = compute_diff(&original, &changed);
        let actual_diffset = DiffSet::try_new(&actual_diff).unwrap();
        let expected_diff = {
            let mut diff = vec![];

            diff.extend_from_slice(&(changed.len() as u32).to_le_bytes());

            diff.extend_from_slice(&(slices.len() as u32).to_le_bytes());

            let mut offset_in_diff = 0u32;
            for (offset_in_account, slice) in slices.iter() {
                diff.extend_from_slice(&offset_in_diff.to_le_bytes());
                diff.extend_from_slice(
                    &(*offset_in_account as u32).to_le_bytes(),
                );
                offset_in_diff += slice.len() as u32;
            }

            for (_, slice) in slices.iter() {
                diff.extend_from_slice(slice);
            }
            diff
        };

        assert_eq!(
            actual_diff.len(),
            expected_diff.len(),
            "number of slices {}",
            slices.len()
        );

        // apply diff back to verify correctness
        let expected_changed = {
            let mut copy = original.clone();
            apply_diff_in_place(&mut copy, &actual_diffset).unwrap();
            copy
        };

        assert_eq!(changed, expected_changed);

        let expected_changed = {
            let mut destination = vec![255; original.len()];
            merge_diff_copy(&mut destination, &original, &actual_diffset)
                .unwrap();
            destination
        };

        assert_eq!(changed, expected_changed);
    }
}
