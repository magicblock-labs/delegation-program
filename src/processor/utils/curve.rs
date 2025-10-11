use solana_curve25519::edwards::{validate_edwards, PodEdwardsPoint};
use solana_program::pubkey::Pubkey;

pub fn is_on_curve(key: &Pubkey) -> bool {
    validate_edwards(&PodEdwardsPoint(key.to_bytes()))
}

pub fn is_on_curve_fast(key: &pinocchio::pubkey::Pubkey) -> bool {
    #[cfg(not(target_os = "solana"))]
    {
        // SAFETY: the layout of pinocchio::pubkey::Pubkey and PodEdwardsPoint is identical
        // so one can be casted to the other without any issue.
        validate_edwards(unsafe { &*(key as *const u8 as *const PodEdwardsPoint) })
    }

    #[cfg(target_os = "solana")]
    {
        // The above unit_test_config-version works great but the following one saves 7 CUs.
        // ref: https://github.com/anza-xyz/agave/blob/aa5cb43d1e/curves/curve25519/src/edwards.rs#L148-L158

        let mut result: u8 = 0;
        let ret = unsafe {
            pinocchio::syscalls::sol_curve_validate_point(
                0, // 0 means Ed25519
                key.as_ptr(),
                &mut result as *mut u8,
            )
        };

        ret == 0
    }
}
