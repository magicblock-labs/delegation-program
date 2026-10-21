use std::cmp::Ordering;

use delegation_program_interface as dpi;
pub(crate) use dpi::diff::{DiffSet, OffsetInData, SizeChanged};
use pinocchio::error::ProgramError;
use wheels::{require_eq, require_le};

use crate::error::DlpError;

/// Detects if there is size change in the changed data.
///  - None               means NO change
///  - Some(size_changed) means the data size has changed and size_changed indicates
///    whether it has expanded or shrunk.
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

    use dpi::diff::{compute_diff, DiffSet};
    use rand::{
        rngs::{OsRng, StdRng},
        seq::IteratorRandom,
        Rng, RngCore, SeedableRng,
    };

    use crate::diff::{apply_diff_in_place, merge_diff_copy};

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
