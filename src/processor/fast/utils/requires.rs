use pinocchio::account_info::AccountInfo;
use pinocchio::program_error::ProgramError;
use pinocchio::pubkey::{pubkey_eq, Pubkey};
use pinocchio_log::log;

use crate::error::DlpError;
use crate::pda::{self, program_config_from_program_id, validator_fees_vault_pda_from_validator};

#[cfg(not(feature = "log-cost"))]
use pinocchio::pubkey;

#[cfg(feature = "log-cost")]
mod pubkey {
    pub use pinocchio::pubkey::log;

    use pinocchio::pubkey::{self, Pubkey};
    use pinocchio::syscalls::sol_remaining_compute_units;
    use pinocchio_log::log;

    #[inline(always)]
    pub fn find_program_address(seeds: &[&[u8]], program_id: &Pubkey) -> (Pubkey, u8) {
        let prev = unsafe { sol_remaining_compute_units() };
        let rv = pubkey::find_program_address(seeds, program_id);
        let curr = unsafe { sol_remaining_compute_units() };
        log!(">> find_program_address => {} CU", prev - curr);
        rv
    }
}

// require true
#[macro_export]
macro_rules! require {
    ($cond:expr, $error:expr) => {{
        if !$cond {
            let expr = stringify!($cond);
            pinocchio_log::log!("require!({}) failed.", expr);
            return Err($error.into());
        }
    }};
}

#[macro_export]
macro_rules! require_signer {
    ($info: expr) => {{
        if !$info.is_signer() {
            log!("require_signer!({}): ", stringify!($info));
            pubkey::log($info.key());
            return Err(ProgramError::MissingRequiredSignature);
        }
    }};
}

// require key1 == key2
#[macro_export]
macro_rules! require_eq_keys {
    ( $key1:expr, $key2:expr, $error:expr) => {{
        if !pinocchio::pubkey::pubkey_eq($key1, $key2) {
            pinocchio_log::log!(
                "require_eq_keys!({}, {}) failed: ",
                stringify!($key1),
                stringify!($key2)
            );
            pinocchio::pubkey::log($key1);
            pinocchio::pubkey::log($key2);
            return Err($error.into());
        }
    }};
}

// require a == b
#[macro_export]
macro_rules! require_eq {
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
macro_rules! require_le {
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
macro_rules! require_lt {
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
macro_rules! require_ge {
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

#[macro_export]
macro_rules! require_n_accounts {
    ( $accounts:expr, $n:literal) => {{
        match $accounts.len().cmp(&$n) {
            core::cmp::Ordering::Less => {
                pinocchio_log::log!(
                    "Need {} accounts, but got less ({}) accounts",
                    $n,
                    $accounts.len()
                );
                return Err(pinocchio::program_error::ProgramError::NotEnoughAccountKeys);
            }
            core::cmp::Ordering::Equal => TryInto::<&[_; $n]>::try_into($accounts)
                .map_err(|_| $crate::error::DlpError::InfallibleError)?,
            core::cmp::Ordering::Greater => {
                pinocchio_log::log!(
                    "Need {} accounts, but got more ({}) accounts",
                    $n,
                    $accounts.len()
                );
                return Err($crate::error::DlpError::TooManyAccountKeys.into());
            }
        }
    }};
}

#[macro_export]
macro_rules! require_some {
    ($option:expr, $error:expr) => {{
        match $option {
            Some(val) => val,
            None => return Err($error.into()),
        }
    }};
}

///
/// require_owned_by(
///     info: &AccountInfo,
///     owner: &Pubkey
/// ) -> Result<(), ProgramError>
///
#[macro_export]
macro_rules! require_owned_by {
    ($info: expr, $owner: expr) => {{
        if !pubkey_eq($info.owner(), $owner) {
            pinocchio_log::log!(
                "require_owned_pda!({}, {})",
                stringify!($info),
                stringify!($owner)
            );
            pubkey::log($info.key());
            pubkey::log($owner);
            return Err(ProgramError::InvalidAccountOwner);
        }
    }};
}

///
/// require_initialized_pda(
///     info: &AccountInfo,
///     seeds: &[&[u8]],
///     program_id: &Pubkey,
///     is_writable: bool,
/// ) -> Result<(), ProgramError> {
///
#[macro_export]
macro_rules! require_initialized_pda {
    ($info:expr, $seeds: expr, $program_id: expr, $is_writable: expr) => {{
        let pda = match pubkey::create_program_address($seeds, $program_id) {
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
        if !pubkey_eq($info.key(), &pda) {
            log!(
                "require_initialized_pda!({}, {}, {}, {}); pubkey_eq failed",
                stringify!($info),
                stringify!($seeds),
                stringify!($program_id),
                stringify!($is_writable)
            );
            pubkey::log($info.key());
            pubkey::log($program_id);
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
            pubkey::log($info.key());
            return Err(ProgramError::Immutable);
        }
    }};
}

#[macro_export]
macro_rules! require_initialized_pda_unsafe {
    ($info:expr, $seeds: expr, $program_id: expr, $is_writable: expr) => {{
        use solana_sha256_hasher::hashv;
        let pda = hashv($seeds).to_bytes();
        if !pubkey_eq($info.key(), &pda) {
            log!(
                "require_initialized_pda!({}, {}, {}, {}); pubkey_eq failed",
                stringify!($info),
                stringify!($seeds),
                stringify!($program_id),
                stringify!($is_writable)
            );
            pubkey::log($info.key());
            pubkey::log($program_id);
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
            pubkey::log($info.key());
            return Err(ProgramError::Immutable);
        }
    }};
}

#[macro_export]
macro_rules! require_pda {
    ($info:expr, $seeds: expr, $program_id: expr, $is_writable: expr) => {{
        let pda = match pubkey::create_program_address($seeds, $program_id) {
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
        if !pubkey_eq($info.key(), &pda) {
            log!(
                "require_pda!({}, {}, {}, {}); pubkey_eq failed",
                stringify!($info),
                stringify!($seeds),
                stringify!($program_id),
                stringify!($is_writable)
            );
            pubkey::log($info.key());
            pubkey::log($program_id);
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
            pubkey::log($info.key());
            return Err(ProgramError::Immutable);
        }
    }};
}

#[macro_export]
macro_rules! require_pda_unsafe {
    ($info:expr, $seeds: expr, $program_id: expr, $is_writable: expr) => {{
        use solana_sha256_hasher::hashv;
        let pda = hashv($seeds).to_bytes();
        if !pubkey_eq($info.key(), &pda) {
            log!(
                "require_pda!({}, {}, {}, {}); pubkey_eq failed",
                stringify!($info),
                stringify!($seeds),
                stringify!($program_id),
                stringify!($is_writable)
            );
            pubkey::log($info.key());
            pubkey::log($program_id);
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
            pubkey::log($info.key());
            return Err(ProgramError::Immutable);
        }
    }};
}

/// pub fn require_program_config(
///     program_config: &AccountInfo,
///     program: &Pubkey,
///     bump: u8,
///     is_writable: bool,
/// ) -> Result<bool, ProgramError> {
#[macro_export]
macro_rules! require_program_config {
    ($program_config: expr, $program: expr, $bump: expr, $is_writable: expr) => {{
        require_pda!(
            $program_config,
            &[pda::PROGRAM_CONFIG_TAG, $program, &[$bump]],
            &crate::fast::ID,
            $is_writable
        );
        !pubkey_eq($program_config.owner(), &pinocchio_system::ID)
    }};
}

#[macro_export]
macro_rules! require_program_config_unsafe {
    ($program_config: expr, $program: expr, $bump: expr, $is_writable: expr) => {{
        crate::require_pda_unsafe!(
            $program_config,
            &[
                pda::PROGRAM_CONFIG_TAG,
                $program,
                &[$bump],
                &crate::fast::ID,
                PDA_MARKER
            ],
            &crate::fast::ID,
            $is_writable
        );
        !pubkey_eq($program_config.owner(), &pinocchio_system::ID)
    }};
}

/// Errors if:
/// - Account is not owned by expected program.
#[inline(always)]
pub fn require_owned_pda(
    info: &AccountInfo,
    owner: &Pubkey,
    label: &str,
) -> Result<(), ProgramError> {
    if !pubkey_eq(info.owner(), owner) {
        log!("Invalid account owner for {}:", label);
        pubkey::log(info.key());
        return Err(ProgramError::InvalidAccountOwner);
    }
    Ok(())
}

/// Errors if:
/// - Account is not a signer.
#[inline(always)]
pub fn require_signer(info: &AccountInfo, label: &str) -> Result<(), ProgramError> {
    if !info.is_signer() {
        log!("Account needs to be signer {}: ", label);
        pubkey::log(info.key());
        return Err(ProgramError::MissingRequiredSignature);
    }

    Ok(())
}

/// Errors if:
/// - Address does not match PDA derived from provided seeds.
#[inline(always)]
pub fn require_pda(
    info: &AccountInfo,
    seeds: &[&[u8]],
    program_id: &Pubkey,
    is_writable: bool,
    label: &str,
) -> Result<u8, ProgramError> {
    let pda = pubkey::find_program_address(seeds, program_id);

    if !pubkey_eq(info.key(), &pda.0) {
        log!("Invalid seeds for {}: ", label);
        pubkey::log(info.key());
        return Err(ProgramError::InvalidSeeds);
    }

    if is_writable && !info.is_writable() {
        log!("Account needs to be writable. Label: {}", label);
        pubkey::log(info.key());
        return Err(ProgramError::Immutable);
    }

    Ok(pda.1)
}

/// Returns true if the account is uninitialized based on the following conditions:
/// - Owner is the system program.
/// - Data is empty.
pub fn is_uninitialized_account(info: &AccountInfo) -> bool {
    pubkey_eq(info.owner(), &pinocchio_system::ID) && info.data_is_empty()
}

/// Errors if:
/// - Owner is not the system program.
/// - Data is not empty.
/// - Account is not writable.
#[inline(always)]
pub fn require_uninitialized_account(
    info: &AccountInfo,
    is_writable: bool,
    ctx: impl RequireUninitializedAccountCtx,
) -> Result<(), ProgramError> {
    if !pubkey_eq(info.owner(), &pinocchio_system::id()) {
        log!(
            "Invalid owner for account. Label: {}; account and owner: ",
            ctx.label()
        );
        pubkey::log(info.key());
        pubkey::log(info.owner());
        return Err(ctx.invalid_account_owner());
    }

    if !info.data_is_empty() {
        log!(
            "Account needs to be uninitialized. Label: {}, account: ",
            ctx.label(),
        );
        pubkey::log(info.key());
        return Err(ctx.account_already_initialized());
    }

    if is_writable && !info.is_writable() {
        log!(
            "Account needs to be writable. label: {}, account: ",
            ctx.label()
        );
        pubkey::log(info.key());
        return Err(ctx.immutable());
    }

    Ok(())
}

/// Errors if:
/// - Address does not match PDA derived from provided seeds.
/// - Cannot load as an uninitialized account.
#[inline(always)]
pub fn require_uninitialized_pda(
    info: &AccountInfo,
    seeds: &[&[u8]],
    program_id: &Pubkey,
    is_writable: bool,
    ctx: impl RequireUninitializedAccountCtx,
) -> Result<u8, ProgramError> {
    let pda = pubkey::find_program_address(seeds, program_id);

    if !pubkey_eq(info.key(), &pda.0) {
        log!("Invalid seeds for account {}: ", ctx.label());
        pubkey::log(info.key());
        return Err(ctx.invalid_seeds());
    }

    require_uninitialized_account(info, is_writable, ctx)?;
    Ok(pda.1)
}

/// Errors if:
/// - Address does not match PDA derived from provided seeds.
/// - Owner is not the expected program.
/// - Account is not writable if set to writable.
pub fn require_initialized_pda(
    info: &AccountInfo,
    seeds: &[&[u8]],
    program_id: &Pubkey,
    is_writable: bool,
    label: &str,
) -> Result<u8, ProgramError> {
    let pda = pubkey::find_program_address(seeds, program_id);
    if !pubkey_eq(info.key(), &pda.0) {
        log!("Invalid seeds (label: {}) for account ", label);
        pubkey::log(info.key());
        return Err(ProgramError::InvalidSeeds);
    }

    require_owned_pda(info, program_id, label)?;

    if is_writable && !info.is_writable() {
        log!("Account needs to be writable. label: {}, account: ", label);
        pubkey::log(info.key());
        return Err(ProgramError::Immutable);
    }

    Ok(pda.1)
}

/// Errors if:
/// - Address does not match the expected value.
/// - Account is not executable.
#[inline(always)]
#[allow(dead_code)]
pub fn require_program(info: &AccountInfo, key: &Pubkey, label: &str) -> Result<(), ProgramError> {
    if !pubkey_eq(info.key(), key) {
        log!("Invalid program account {}: ", label);
        pubkey::log(info.key());
        return Err(ProgramError::IncorrectProgramId);
    }

    if !info.executable() {
        log!("{} program is not executable: ", label);
        pubkey::log(info.key());
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(())
}

/// Load fee vault PDA
/// - Protocol fees vault PDA
pub fn require_initialized_protocol_fees_vault(
    fees_vault: &AccountInfo,
    is_writable: bool,
) -> Result<(), ProgramError> {
    require_initialized_pda(
        fees_vault,
        &[b"fees-vault"],
        &crate::fast::ID,
        is_writable,
        "protocol fees vault",
    )?;
    Ok(())
}

/// Load validator fee vault PDA
/// - Validator fees vault PDA must be derived from the validator pubkey
/// - Validator fees vault PDA must be initialized with the expected seeds and owner
pub fn require_initialized_validator_fees_vault(
    validator: &AccountInfo,
    validator_fees_vault: &AccountInfo,
    is_writable: bool,
) -> Result<(), ProgramError> {
    let pda = validator_fees_vault_pda_from_validator(&(*validator.key()).into());
    if !pubkey_eq(validator_fees_vault.key(), pda.as_array()) {
        log!("Invalid validator fees vault PDA, expected: ");
        pubkey::log(pda.as_array());
        log!("but got: ");
        pubkey::log(validator_fees_vault.key());
        return Err(DlpError::InvalidAuthority.into());
    }
    require_initialized_pda(
        validator_fees_vault,
        &[pda::VALIDATOR_FEES_VAULT_TAG, validator.key()],
        &crate::fast::ID,
        is_writable,
        "validator fees vault",
    )?;
    Ok(())
}

/// Load program config PDA
/// - Program config PDA must be initialized with the expected seeds and owner, or not exists
pub fn require_program_config(
    program_config: &AccountInfo,
    program: &Pubkey,
    is_writable: bool,
) -> Result<bool, ProgramError> {
    let pda = program_config_from_program_id(&(*program).into());
    if !pubkey_eq(pda.as_array(), program_config.key()) {
        log!("Invalid program config PDA, expected: ");
        pubkey::log(pda.as_array());
        log!("but got: ");
        pubkey::log(program_config.key());
        return Err(DlpError::InvalidAuthority.into());
    }
    require_pda(
        program_config,
        &[pda::PROGRAM_CONFIG_TAG, program],
        &crate::fast::ID,
        is_writable,
        "program config",
    )?;
    Ok(!pubkey_eq(program_config.owner(), &pinocchio_system::ID))
}

/// Load initialized delegation record
/// - Delegation record must be derived from the delegated account
pub fn require_initialized_delegation_record(
    delegated_account: &AccountInfo,
    delegation_record: &AccountInfo,
    is_writable: bool,
) -> Result<(), ProgramError> {
    require_initialized_pda(
        delegation_record,
        &[pda::DELEGATION_RECORD_TAG, delegated_account.key()],
        &crate::fast::ID,
        is_writable,
        "delegation record",
    )?;
    Ok(())
}

/// Load initialized delegation metadata
/// - Delegation metadata must be derived from the delegated account
pub fn require_initialized_delegation_metadata(
    delegated_account: &AccountInfo,
    delegation_metadata: &AccountInfo,
    is_writable: bool,
) -> Result<(), ProgramError> {
    require_initialized_pda(
        delegation_metadata,
        &[pda::DELEGATION_METADATA_TAG, delegated_account.key()],
        &crate::fast::ID,
        is_writable,
        "delegation metadata",
    )?;
    Ok(())
}

/// Load initialized commit state account
/// - Commit state account must be derived from the delegated account pubkey
pub fn require_initialized_commit_state(
    delegated_account: &AccountInfo,
    commit_state: &AccountInfo,
    is_writable: bool,
) -> Result<(), ProgramError> {
    require_initialized_pda(
        commit_state,
        &[pda::COMMIT_STATE_TAG, delegated_account.key()],
        &crate::fast::ID,
        is_writable,
        "commit state",
    )?;
    Ok(())
}

/// Load initialized commit state record
/// - Commit record account must be derived from the delegated account pubkey
pub fn require_initialized_commit_record(
    delegated_account: &AccountInfo,
    commit_record: &AccountInfo,
    is_writable: bool,
) -> Result<(), ProgramError> {
    require_initialized_pda(
        commit_record,
        &[pda::COMMIT_RECORD_TAG, delegated_account.key()],
        &crate::fast::ID,
        is_writable,
        "commit record",
    )?;
    Ok(())
}

/// Context for `require_uninitialized_account` / `require_uninitialized_pda`.
///
/// This trait describes how to map low–level validation failures for a
/// particular account (e.g. "commit state account", "delegation record")
/// into concrete `DlpError` variants.
pub(crate) trait RequireUninitializedAccountCtx {
    fn label(&self) -> &str;
    fn invalid_seeds(&self) -> ProgramError;
    fn invalid_account_owner(&self) -> ProgramError;
    fn account_already_initialized(&self) -> ProgramError;
    fn immutable(&self) -> ProgramError;
}

macro_rules! define_uninitialized_ctx {
    (
        $name:ident,
        label = $label:expr,
        invalid_seeds = $seeds:expr,
        invalid_account_owner = $owner:expr,
        account_already_initialized = $already_init:expr,
        immutable = $immutable:expr
    ) => {
        pub(crate) struct $name;

        impl $crate::processor::fast::utils::requires::RequireUninitializedAccountCtx for $name {
            fn label(&self) -> &str {
                $label
            }

            fn invalid_seeds(&self) -> pinocchio::program_error::ProgramError {
                $seeds.into()
            }

            fn invalid_account_owner(&self) -> pinocchio::program_error::ProgramError {
                $owner.into()
            }

            fn account_already_initialized(&self) -> pinocchio::program_error::ProgramError {
                $already_init.into()
            }

            fn immutable(&self) -> pinocchio::program_error::ProgramError {
                $immutable.into()
            }
        }
    };
}

define_uninitialized_ctx!(
    CommitStateAccountCtx,
    label = "commit state account",
    invalid_seeds = DlpError::CommitStateInvalidSeeds,
    invalid_account_owner = DlpError::CommitStateInvalidAccountOwner,
    account_already_initialized = DlpError::CommitStateAlreadyInitialized,
    immutable = DlpError::CommitStateImmutable
);

define_uninitialized_ctx!(
    CommitRecordCtx,
    label = "commit record",
    invalid_seeds = DlpError::CommitRecordInvalidSeeds,
    invalid_account_owner = DlpError::CommitRecordInvalidAccountOwner,
    account_already_initialized = DlpError::CommitRecordAlreadyInitialized,
    immutable = DlpError::CommitRecordImmutable
);

define_uninitialized_ctx!(
    DelegationRecordCtx,
    label = "delegation record",
    invalid_seeds = DlpError::DelegationRecordInvalidSeeds,
    invalid_account_owner = DlpError::DelegationRecordInvalidAccountOwner,
    account_already_initialized = DlpError::DelegationRecordAlreadyInitialized,
    immutable = DlpError::DelegationRecordImmutable
);

define_uninitialized_ctx!(
    DelegationMetadataCtx,
    label = "delegation metadata",
    invalid_seeds = DlpError::DelegationMetadataInvalidSeeds,
    invalid_account_owner = DlpError::DelegationMetadataInvalidAccountOwner,
    account_already_initialized = DlpError::DelegationMetadataAlreadyInitialized,
    immutable = DlpError::DelegationMetadataImmutable
);

define_uninitialized_ctx!(
    UndelegateBufferCtx,
    label = "undelegate buffer",
    invalid_seeds = DlpError::UndelegateBufferInvalidSeeds,
    invalid_account_owner = DlpError::UndelegateBufferInvalidAccountOwner,
    account_already_initialized = DlpError::UndelegateBufferAlreadyInitialized,
    immutable = DlpError::UndelegateBufferImmutable
);

pub fn require_authorization(
    program_data: &AccountInfo,
    admin: &AccountInfo,
) -> Result<(), ProgramError> {
    #[cfg(feature = "unit_test_config")]
    {
        let _ = program_data;

        require_eq_keys!(
            &crate::consts::DEFAULT_VALIDATOR_IDENTITY.to_bytes().into(),
            admin.key(),
            ProgramError::IncorrectAuthority
        );
        return Ok(());
    }

    #[cfg(not(feature = "unit_test_config"))]
    {
        // Derive and validate program data address
        require_eq_keys!(
            &crate::consts::DELEGATION_PROGRAM_DATA_ID,
            program_data.key(),
            ProgramError::IncorrectAuthority
        );

        //
        // ref: https://github.com/anza-xyz/solana-sdk/blob/55809cfe/loader-v3-interface/src/state.rs
        let offset_of_upgrade_authority_address =
            4 // for the variant ProgramData (u32)
                + 8 // for slot (u64)
            ;

        // constants to enhance readability
        const PROGRAM_DATA: u8 = 3;
        const OPTION_SOME: u8 = 1;

        //
        // SAFETY: This authorization logic reads raw ProgramData bytes using the current
        // Upgradeable Loader v3 layout.
        let data = program_data.try_borrow_data()?;
        if data.len() >= offset_of_upgrade_authority_address + 33
            && data[0] == PROGRAM_DATA
            && data[offset_of_upgrade_authority_address] == OPTION_SOME
        {
            let bytes = &data
                [offset_of_upgrade_authority_address + 1..offset_of_upgrade_authority_address + 33];

            let upgrade_authority_address =
                unsafe { *(bytes.as_ptr() as *const pinocchio::pubkey::Pubkey) };

            require_eq_keys!(
                &upgrade_authority_address,
                admin.key(),
                ProgramError::IncorrectAuthority
            );

            Ok(())
        } else {
            Err(ProgramError::InvalidAccountData)
        }
    }
}
