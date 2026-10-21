use std::cmp::{min, Ordering};

use delegation_program_interface::diff::{
    SIZE_OF_CHANGED_LEN, SIZE_OF_NUM_OFFSET_PAIRS, SIZE_OF_SINGLE_OFFSET_PAIR,
};
use rkyv::util::AlignedVec;

/// Computes the encoded changed segments between two account-data buffers.
///
/// The returned buffer is aligned for `DiffSet` zero-copy decoding.
pub fn compute_diff(original: &[u8], changed: &[u8]) -> AlignedVec {
    let mut diffs: Vec<(usize, &[u8])> = Vec::new();
    let min_len = min(original.len(), changed.len());
    let mut diff_size = 0;
    let mut index = 0;

    while index < min_len {
        if original[index] != changed[index] {
            let start = index;
            while index < min_len && original[index] != changed[index] {
                index += 1;
            }
            diffs.push((start, &changed[start..index]));
            diff_size += index - start;
        } else {
            index += 1;
        }
    }

    match changed.len().cmp(&original.len()) {
        Ordering::Greater => {
            diffs.push((original.len(), &changed[original.len()..]));
            diff_size += changed.len() - original.len();
        }
        Ordering::Less | Ordering::Equal => {}
    }

    let mut output = AlignedVec::with_capacity(
        SIZE_OF_CHANGED_LEN
            + SIZE_OF_NUM_OFFSET_PAIRS
            + SIZE_OF_SINGLE_OFFSET_PAIR * diffs.len()
            + diff_size,
    );

    output.extend_from_slice(&(changed.len() as u32).to_le_bytes());
    output.extend_from_slice(&(diffs.len() as u32).to_le_bytes());

    let mut offset_in_diff = 0u32;
    for (offset_in_account, slice) in &diffs {
        output.extend_from_slice(&offset_in_diff.to_le_bytes());
        output.extend_from_slice(&(*offset_in_account as u32).to_le_bytes());
        offset_in_diff += slice.len() as u32;
    }

    for (_, slice) in diffs {
        output.extend_from_slice(slice);
    }

    output
}

#[cfg(test)]
mod tests {
    use delegation_program_interface::diff::DiffSet;

    use super::compute_diff;

    #[test]
    fn computes_parseable_diff() {
        let original = [0, 1, 2, 3, 4];
        let changed = [0, 9, 2, 8, 4, 7];
        let diff = compute_diff(&original, &changed);
        let parsed = DiffSet::try_new(&diff).unwrap();

        assert_eq!(parsed.changed_len(), changed.len());
        assert_eq!(parsed.segments_count(), 3);
    }
}
