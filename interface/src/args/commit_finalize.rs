use std::ptr::read_unaligned;

use wheels::variable_offset_layout;

use crate::compat::Pubkey;

#[derive(Clone, Debug, PartialEq, Eq)]
#[variable_offset_layout(buffer_offset = 1)]
pub struct CommitFinalizeArgs {
    /// Next commit sequence number for this delegated account.
    pub commit_seq: u32,

    /// Account lamports on the ER after this commit.
    pub lamports: u64,

    /// Account owner committed by the ER.
    pub owner: Pubkey,

    /// Whether the validator requests undelegation after this commit.
    pub allow_undelegation: bool,

    /// Stored as `CommitPayloadKind::value()`.
    pub payload_kind: u8,

    /// Keeps the inline payload four-byte aligned for zero-copy diff decoding.
    pub payload_alignment_padding: u8,

    /// Complete account data or an encoded diff, according to `payload_kind`.
    #[flexible = 4]
    pub payload: Vec<u8>,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommitPayloadKind {
    FullData = 0,
    Diff = 1,
}

impl CommitPayloadKind {
    pub const fn value(self) -> u8 {
        self as u8
    }
}

impl<'a> CommitFinalizeArgsView<'a>
where
    Pubkey: ::bytemuck::Pod,
{
    #[inline(always)]
    pub fn commit_seq_x(&self) -> u32 {
        const OFFSET: usize = 0;
        unsafe { read_unaligned(self.bytes.as_ptr().add(OFFSET) as *const u32) }
    }

    #[inline(always)]
    pub fn lamports_x(&self) -> u64 {
        const OFFSET: usize = 4;
        // save 3 CU
        unsafe { read_unaligned(self.bytes.as_ptr().add(OFFSET) as *const u64) }

        // consume 3 CU more compared to read_unaligned
        //u64::from_le_bytes(self.bytes[OFFSET..OFFSET + 8].try_into().unwrap())
    }

    #[inline(always)]
    pub fn owner_x(&self) -> &Pubkey {
        const OFFSET: usize = 12;
        //::bytemuck::from_bytes::<Pubkey>(&self.bytes[OFFSET..OFFSET + 32])

        // saves 12 CU compared to the above
        unsafe { &*(self.bytes.as_ptr().add(OFFSET) as *const Pubkey) }
    }

    // inlining is weird. it does not always help. it didn't help earlier 3 fns.
    // but here, it saves almost 13 CU.
    // that only implies that previous 3 fns are inlined regardless, so inline(always)
    // does not have any affect, which I validated by using inline(never) on owner_x()
    // and it did increase it by 13 CU. Interesting.
    #[inline(always)]
    pub fn allow_undelegation_x(&self) -> bool {
        //let offset = 44;
        //(&self.bytes[offset..offset + 1])[0] != 0
        //self.bytes[offset] != 0
        unsafe { *self.bytes.get_unchecked(44) != 0 }
    }

    #[inline(always)]
    pub fn payload_kind_x(&self) -> u8 {
        unsafe { *self.bytes.get_unchecked(45) }
    }

    #[inline(always)]
    pub fn payload_x(&self) -> &'a [u8] {
        unsafe { self.bytes.get_unchecked(51..) }
    }
}
