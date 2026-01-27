use pinocchio::error::ProgramError;

/// SAFETY
///
/// This program uses POD structs for account and instruction data that are
/// read/written via raw byte copies (bytemuck, raw pointers, or direct memory
/// views). This document establishes the safety and correctness rules for such
/// usage, with special attention to endianness, alignment, and representation.
///
/// Solana BPF Memory Model & Endianness Guarantees
/// ===============================================
///
/// The Solana BPF VM uses a little-endian memory model. As a result, all
/// multi-byte scalar types (integers and floats) are stored in memory using
/// little-endian byte order.
///
/// When this program serializes POD types by copying their raw memory layout
/// (e.g. via bytemuck::bytes_of or ptr::copy_nonoverlapping), the resulting
/// byte sequence is also little-endian, because it directly reflects the
/// machine-level representation.
///
/// But also note that although the Solana VM is little-endian, Solana does not mandate any
/// serialization format or endianness for account or instruction data. A program
/// may choose to serialize its data in any order it wants. The only rule is: the
/// reader must interpret the data consistently with the writer.
///
/// However, in this codebase we serialize POD types by copying their raw memory
/// bytes directly. Since the machine representation is little-endian, this implicitly
/// produces a little-endian serialization format.
///
/// Alignment
/// =========
///
/// POD structs rely on correct alignment whenever they are accessed through "typed"
/// pointers i.e both during serialization and deserialization (even though in POD's case,
/// serialization/deserialization are merely reinterpretion of the raw memory).
///
/// That basically means, if a struct has alignment n, then:
///
/// - The starting address of account buffer must be aligned to n-byte boundary.
/// - The struct's fields must not cause unintended padding beyond what #[repr(C)]
///   automatically defines and each field must be aligned to their own alignment.
///   - In our codebase, we however explicitly define the padding for two reasons:
///     - The padding is visible.
///     - And since the padding is visible and has a name, we can easily initialize it
///       with all zeroes without using any dirty trick. Note that uninitialized paddings
///       are dangerous and would invoke UB (Rust must be inheriting this UB behaviour
///       from C++ indirectly through LLVM).
///
/// Our codebase assumes that both `account data` and `instruction data` are aligned to
/// 8-byte boundary and therefore all structs/types have either 8-byte alignment requirement
/// or weaker one. The `bytemuck` crates enforces the alignment requirements, so we do not
/// have to do it ourselves.
///
/// Avoid char and bool
/// ===================
///
/// Two Rust primitives must be AVOIDED in low-level serialization and POD
/// layouts:
///   - char
///   - bool
///
/// These two are "disgusting types" in the context of raw byte-level
/// programming because NOT all bit-patterns are valid values for them:
///   - bool only permits 0 and 1; all other bytes are invalid.
///   - char permits only valid Unicode scalar values; most u32 patterns
///     represent invalid char.
///
/// As a result, neither char nor bool can be safely reinterpreted from
/// raw bytes, cannot be Pod, and must not appear in account or instruction
/// layouts. Use u8 instead. Note that `#[repr(u8)] enum` is disgusting
/// type as well, so we cannot use that either.
///
/// Ref: https://docs.rs/bytemuck/latest/bytemuck/trait.Pod.html
///
pub trait PodView {
    /// The exact size of the POD type in bytes.
    ///
    /// This is used to create new accounts and to ensure that the buffer
    /// we write to or read from matches the expected layout exactly.
    const SPACE: usize;
    const ALIGN: usize;

    fn to_bytes(&self) -> Vec<u8>
    where
        Self: bytemuck::Pod,
    {
        bytemuck::bytes_of(self).to_vec()
    }

    /// Copy the raw bytes of Self into the given mutable buffer.
    ///
    /// This is a *copy*, not a cast. The buffer must be exactly Self::SPACE
    /// bytes long. On success, the buffer will contain a byte-for-byte
    /// representation of this struct, else it will return ErrorCode::SizeMismatch.
    fn try_copy_to(&self, buffer: &mut [u8]) -> Result<(), ProgramError>;

    /// This function performs a zero-copy cast from [u8] to &Self.
    ///
    /// The buffer must:
    ///   - be exactly Self::SPACE bytes long
    ///   - be properly aligned (guaranteed for Solana account data)
    ///   - contain valid POD bytes for Self
    fn try_view_from(buffer: &[u8]) -> Result<&Self, ProgramError>;

    /// Mutable version of try_view_from.
    fn try_view_from_mut(buffer: &mut [u8]) -> Result<&mut Self, ProgramError>;

    /// During testing, the account/ix data may not be properly aligned.
    /// In that case, we could create a "copy" instead of a "view" using this
    /// function that takes care of provided possibly-unaligned_buffer.
    #[cfg(feature = "unit_test_config")]
    fn try_from_unaligned(
        unaligned_buffer: &[u8],
    ) -> Result<Self, ProgramError>
    where
        Self: Sized;
}

impl<T: bytemuck::Pod> PodView for T {
    const SPACE: usize = core::mem::size_of::<T>();
    const ALIGN: usize = core::mem::align_of::<T>();

    fn try_copy_to(&self, buffer: &mut [u8]) -> Result<(), ProgramError> {
        if buffer.len() != Self::SPACE {
            return Err(ProgramError::InvalidArgument);
        }
        let src = bytemuck::bytes_of(self);
        buffer.copy_from_slice(src);
        Ok(())
    }

    fn try_view_from(buffer: &[u8]) -> Result<&Self, ProgramError> {
        bytemuck::try_from_bytes(buffer)
            .map_err(|_| ProgramError::InvalidArgument)
    }

    fn try_view_from_mut(buffer: &mut [u8]) -> Result<&mut Self, ProgramError> {
        bytemuck::try_from_bytes_mut(buffer)
            .map_err(|_| ProgramError::InvalidArgument)
    }

    #[cfg(feature = "unit_test_config")]
    fn try_from_unaligned(
        possibly_unaligned_buffer: &[u8],
    ) -> Result<Self, ProgramError> {
        bytemuck::try_pod_read_unaligned(possibly_unaligned_buffer)
            .map_err(|_| ProgramError::InvalidArgument)
    }
}
