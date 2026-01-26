use pinocchio::Address;

use core::ptr::read;

// require true
#[macro_export]
macro_rules! v2_require {
    ($cond:expr, $error:expr) => {{
        if !$cond {
            let expr = stringify!($cond);
            pinocchio_log::log!("require!({}) failed.", expr);
            return Err($error.into());
        }
    }};
}

// require (info.is_signer())
#[macro_export]
macro_rules! v2_require_signer {
    ($info: expr) => {{
        if !$info.is_signer() {
            pinocchio_log::log!("require_signer!({}): ", stringify!($info));
            $info.address().log();
            return Err(ProgramError::MissingRequiredSignature);
        }
    }};
}

// require key1 == key2
#[macro_export]
macro_rules! v2_require_eq_keys {
    ( $key1:expr, $key2:expr, $error:expr) => {{
        if !pinocchio::address::address_eq($key1, $key2) {
            pinocchio_log::log!(
                "require_eq_keys!({}, {}) failed: ",
                stringify!($key1),
                stringify!($key2)
            );
            $key1.log();
            $key2.log();
            return Err($error.into());
        }
    }};
}

#[inline(always)]
pub unsafe fn unsafe_address_eq(a1: &Address, a2: &Address) -> bool {
    if true {
        let p1_ptr = a1.as_array().as_ptr().cast::<u64>();
        let p2_ptr = a2.as_array().as_ptr().cast::<u64>();

        read(p1_ptr) == read(p2_ptr)
            && read(p1_ptr.add(1)) == read(p2_ptr.add(1))
            && read(p1_ptr.add(2)) == read(p2_ptr.add(2))
            && read(p1_ptr.add(3)) == read(p2_ptr.add(3))
    } else {
        let a1: &[[u8; 8]; 4] = bytemuck::cast_ref(a1.as_array());
        let a2: &[[u8; 8]; 4] = bytemuck::cast_ref(a2.as_array());

        u64::from_le_bytes(a1[0]) == u64::from_le_bytes(a2[0])
            && u64::from_le_bytes(a1[1]) == u64::from_le_bytes(a2[1])
            && u64::from_le_bytes(a1[2]) == u64::from_le_bytes(a2[2])
            && u64::from_le_bytes(a1[3]) == u64::from_le_bytes(a2[3])
    }
}
// require key1 == key2
#[macro_export]
macro_rules! v2_require_eq_keys_unsafe {
    ( $key1:expr, $key2:expr, $error:expr) => {{
        if !$crate::v2::requires::unsafe_address_eq($key1, $key2) {
            pinocchio_log::log!(
                "require_eq_keys!({}, {}) failed: ",
                stringify!($key1),
                stringify!($key2)
            );
            $key1.log();
            $key2.log();
            return Err($error.into());
        }
    }};
}

// require a == b
#[macro_export]
macro_rules! v2_require_eq {
    ( $val1:expr, $val2:expr, $error:expr) => {{
        if !($val1 == $val2) {
            pinocchio_log::log!(
                "require_eq!({}, {}) failed: {} == {}",
                stringify!($val1),
                stringify!($val2),
                $val1,
                $val2
            );
            return Err($error.into());
        }
    }};
}

// require a <= b
#[macro_export]
macro_rules! v2_require_le {
    ( $val1:expr, $val2:expr, $error:expr) => {{
        if !($val1 <= $val2) {
            pinocchio_log::log!(
                "require_le!({}, {}) failed: {} <= {}",
                stringify!($val1),
                stringify!($val2),
                $val1,
                $val2
            );
            return Err($error.into());
        }
    }};
}

// require a < b
#[macro_export]
macro_rules! v2_require_lt {
    ( $val1:expr, $val2:expr, $error:expr) => {{
        if !($val1 < $val2) {
            pinocchio_log::log!(
                "require_lt!({}, {}) failed: {} < {}",
                stringify!($val1),
                stringify!($val2),
                $val1,
                $val2
            );
            return Err($error.into());
        }
    }};
}

// require a >= b
#[macro_export]
macro_rules! v2_require_ge {
    ( $val1:expr, $val2:expr, $error:expr) => {{
        if !($val1 >= $val2) {
            pinocchio_log::log!(
                "require_ge!({}, {}) failed: {} >= {}",
                stringify!($val1),
                stringify!($val2),
                $val1,
                $val2
            );
            return Err($error.into());
        }
    }};
}

// require a > b
#[macro_export]
macro_rules! v2_require_gt {
    ( $val1:expr, $val2:expr, $error:expr) => {{
        if !($val1 > $val2) {
            pinocchio_log::log!(
                "require_gt!({}, {}) failed: {} > {}",
                stringify!($val1),
                stringify!($val2),
                $val1,
                $val2
            );
            return Err($error.into());
        }
    }};
}

#[macro_export]
macro_rules! v2_require_n_accounts {
    ( $accounts:expr, $n:literal) => {{
        let n = $accounts.len();
        if n == $n {
            TryInto::<&[_; $n]>::try_into($accounts)
                .map_err(|_| $crate::error::DlpError::InfallibleError)?
        } else if n < $n {
            pinocchio_log::log!(
                "Need {} accounts, but got less ({}) accounts",
                $n,
                $accounts.len()
            );
            return Err(pinocchio::error::ProgramError::NotEnoughAccountKeys);
        } else {
            pinocchio_log::log!(
                "Need {} accounts, but got more ({}) accounts",
                $n,
                $accounts.len()
            );
            return Err($crate::error::DlpError::TooManyAccountKeys.into());
        }
    }};
}

#[macro_export]
macro_rules! v2_require_some {
    ($option:expr, $error:expr) => {{
        match $option {
            Some(val) => val,
            None => return Err($error.into()),
        }
    }};
}

///
/// require_owned_by(
///     info: &AccountView,
///     owner: &Address
/// ) -> Result<(), ProgramError>
///
#[macro_export]
macro_rules! v2_require_owned_by {
    ($info: expr, $owner: expr) => {{
        if !pinocchio::address::address_eq(unsafe { $info.owner() }, $owner) {
            pinocchio_log::log!(
                "require_owned_by!({}, {})",
                stringify!($info),
                stringify!($owner)
            );
            $info.address().log();
            $owner.log();
            return Err(ProgramError::InvalidAccountOwner);
        }
    }};
}

#[macro_export]
macro_rules! v2_require_uninitialized_pda {
    ($info:expr, $seeds: expr) => {{
        let pda = match pinocchio::Address::create_program_address($seeds, &$crate::fast::ID) {
            Ok(pda) => pda,
            Err(_) => {
                log!(
                    "require_uninitialized_pda!({}, {}); create_program_address failed",
                    stringify!($info),
                    stringify!($seeds)
                );
                return Err(ProgramError::InvalidSeeds);
            }
        };
        if !address_eq($info.address(), &pda) {
            log!(
                "require_uninitialized_pda!({}, {}); address_eq failed",
                stringify!($info),
                stringify!($seeds),
            );
            $info.address().log();
            return Err(ProgramError::InvalidSeeds);
        }

        v2_require_owned_by!($info, &$crate::fast::ID);

        if $info.is_writable() {
            log!(
                "require_initialized_pda!({}, {}); is_writable expectation failed",
                stringify!($info),
                stringify!($seeds),
            );
            $info.address().log();
            return Err(ProgramError::Immutable);
        }
    }};
}

///
/// require_initialized_pda(
///     info: &AccountView,
///     seeds: &[&[u8]],
///     program_id: &Address,
///     is_writable: bool,
/// ) -> Result<(), ProgramError> {
///
#[macro_export]
macro_rules! v2_require_initialized_pda {
    ($info:expr, $seeds: expr, $program_id: expr, $is_writable: expr) => {{
        let pda = match pinocchio::Address::create_program_address($seeds, $program_id) {
            Ok(pda) => pda,
            Err(_) => {
                log!(
                    "require_initialized_pda!({}, {}, {}, {}); create_program_address failed",
                    stringify!($info),
                    stringify!($seeds),
                    stringify!($program_id),
                    stringify!($is_writable),
                );
                return Err(ProgramError::InvalidSeeds);
            }
        };
        if !address_eq($info.address(), &pda) {
            log!(
                "require_initialized_pda!({}, {}, {}, {}); address_eq failed",
                stringify!($info),
                stringify!($seeds),
                stringify!($program_id),
                stringify!($is_writable)
            );
            $info.address().log();
            $program_id.log();
            return Err(ProgramError::InvalidSeeds);
        }

        require_owned_by!($info, $program_id);

        if $is_writable && !$info.is_writable() {
            log!(
                "require_initialized_pda!({}, {}, {}, {}); is_writable expectation failed",
                stringify!($info),
                stringify!($seeds),
                stringify!($program_id),
                stringify!($is_writable)
            );
            $info.address().log();
            return Err(ProgramError::Immutable);
        }
    }};
}

#[macro_export]
macro_rules! v2_require_initialized_pda_fast {
    ($info:expr, $seeds: expr, $is_writable: expr) => {{
        use solana_sha256_hasher::hashv;
        let pda = hashv($seeds).to_bytes().into();
        if !address_eq($info.address(), &pda) {
            log!(
                "require_initialized_pda!({}, {}, {}); address_eq failed",
                stringify!($info),
                stringify!($seeds),
                stringify!($is_writable)
            );
            $info.address().log();
            return Err(ProgramError::InvalidSeeds);
        }

        require_owned_by!($info, &$crate::fast::ID);

        if $is_writable && !$info.is_writable() {
            log!(
                "require_initialized_pda!({}, {}, {}); is_writable expectation failed",
                stringify!($info),
                stringify!($seeds),
                stringify!($is_writable)
            );
            $info.address().log();
            return Err(ProgramError::Immutable);
        }
    }};
}

#[macro_export]
macro_rules! v2_require_pda {
    ($info:expr, $seeds: expr, $program_id: expr, $is_writable: expr) => {{
        let pda = match pinocchio::Address::create_program_address($seeds, $program_id) {
            Ok(pda) => pda,
            Err(_) => {
                log!(
                    "require_pda!({}, {}, {}, {}); create_program_address failed",
                    stringify!($info),
                    stringify!($seeds),
                    stringify!($program_id),
                    stringify!($is_writable),
                );
                return Err(ProgramError::InvalidSeeds);
            }
        };
        if !address_eq($info.address(), &pda) {
            log!(
                "require_pda!({}, {}, {}, {}); address_eq failed",
                stringify!($info),
                stringify!($seeds),
                stringify!($program_id),
                stringify!($is_writable)
            );
            $info.address().log();
            $program_id.log();
            return Err(ProgramError::InvalidSeeds);
        }

        if $is_writable && !$info.is_writable() {
            log!(
                "require_pda!({}, {}, {}, {}); is_writable expectation failed",
                stringify!($info),
                stringify!($seeds),
                stringify!($program_id),
                stringify!($is_writable)
            );
            $info.address().log();
            return Err(ProgramError::Immutable);
        }
    }};
}

#[macro_export]
macro_rules! v2_require_pda_fast {
    ($info:expr, $seeds: expr, $is_writable: expr) => {{
        use solana_sha256_hasher::hashv;
        let pda = hashv($seeds).to_bytes().into();
        if !address_eq($info.address(), &pda) {
            log!(
                "require_pda!({}, {}, {}); address_eq failed",
                stringify!($info),
                stringify!($seeds),
                stringify!($is_writable)
            );
            $info.address().log();
            return Err(ProgramError::InvalidSeeds);
        }

        if $is_writable && !$info.is_writable() {
            log!(
                "require_pda!({}, {}, {}); is_writable expectation failed",
                stringify!($info),
                stringify!($seeds),
                stringify!($is_writable)
            );
            $info.address().log();
            return Err(ProgramError::Immutable);
        }
    }};
}

/// pub fn require_program_config(
///     program_config: &AccountView,
///     program: &Address,
///     bump: u8,
///     is_writable: bool,
/// ) -> Result<bool, ProgramError> {
#[macro_export]
macro_rules! v2_require_program_config {
    ($program_config: expr, $program: expr, $bump: expr, $is_writable: expr) => {{
        $crate::require_pda!(
            $program_config,
            &[pda::PROGRAM_CONFIG_TAG, $program.as_ref(), &[$bump]],
            &$crate::fast::ID,
            $is_writable
        );
        !address_eq(unsafe { $program_config.owner() }, &pinocchio_system::ID)
    }};
}

/// pub fn require_program_config(
///     program_config: &AccountView,
///     program: &Address,
///     bump: u8,
///     is_writable: bool,
/// ) -> Result<bool, ProgramError> {
#[macro_export]
macro_rules! v2_require_program_config_fast {
    ($program_config: expr, $program: expr, $bump: expr, $is_writable: expr) => {{
        $crate::require_pda_fast!(
            $program_config,
            &[
                pda::PROGRAM_CONFIG_TAG,
                $program.as_ref(),
                &[$bump],
                &$crate::fast::ID.as_ref(),
                PDA_MARKER
            ],
            $is_writable
        );
        !address_eq(unsafe { $program_config.owner() }, &pinocchio_system::ID)
    }};
}
