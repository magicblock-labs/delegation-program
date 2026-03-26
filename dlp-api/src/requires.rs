use pinocchio::{
    address::{address_eq, Address},
    error::ProgramError,
    AccountView,
};
use pinocchio_log::log;

use crate::{
    error::DlpError,
    pda::{
        self, program_config_from_program_id,
        validator_fees_vault_pda_from_validator,
    },
};

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

// require (info.is_signer())
#[macro_export]
macro_rules! require_signer {
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
macro_rules! require_eq_keys {
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

// require a > b
#[macro_export]
macro_rules! require_gt {
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
macro_rules! require_n_accounts {
    ( $accounts:expr, $n:literal) => {{
        match $accounts.len().cmp(&$n) {
            core::cmp::Ordering::Less => {
                pinocchio_log::log!(
                    "Need {} accounts, but got less ({}) accounts",
                    $n,
                    $accounts.len()
                );
                return Err(
                    pinocchio::error::ProgramError::NotEnoughAccountKeys,
                );
            }
            core::cmp::Ordering::Equal => {
                TryInto::<&[_; $n]>::try_into($accounts)
                    .map_err(|_| $crate::error::DlpError::InfallibleError)?
            }
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
macro_rules! require_n_accounts_with_optionals {
    ( $accounts:expr, $n:literal) => {{
        match $accounts.len().cmp(&$n) {
            core::cmp::Ordering::Less => {
                pinocchio_log::log!(
                    "Need {} accounts, but got less ({}) accounts",
                    $n,
                    $accounts.len()
                );
                return Err(
                    pinocchio::error::ProgramError::NotEnoughAccountKeys,
                );
            }
            _ => {
                let (exact, optionals) = $accounts.split_at($n);

                (
                    TryInto::<&[_; $n]>::try_into(exact).map_err(|_| {
                        $crate::error::DlpError::InfallibleError
                    })?,
                    optionals,
                )
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
///     info: &AccountView,
///     owner: &Address
/// ) -> Result<(), ProgramError>
///
#[macro_export]
macro_rules! require_owned_by {
    ($info: expr, $owner: expr) => {{
        if !address_eq(unsafe { $info.owner() }, $owner) {
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

///
/// require_initialized_pda(
///     info: &AccountView,
///     seeds: &[&[u8]],
///     program_id: &Address,
///     is_writable: bool,
/// ) -> Result<(), ProgramError> {
///
#[macro_export]
macro_rules! require_initialized_pda {
    ($info:expr, $seeds: expr, $program_id: expr, $is_writable: expr) => {{
        let pda = match pinocchio::Address::create_program_address($seeds, $program_id) {
            Ok(pda) => pda,
            Err(_) => {
                pinocchio_log::log!(
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
            pinocchio_log::log!(
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
            pinocchio_log::log!(
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
macro_rules! require_initialized_pda_fast {
    ($info:expr, $seeds: expr, $is_writable: expr) => {{
        let pda = solana_sha256_hasher::hashv($seeds).to_bytes();
        if !address_eq($info.address(), &pda.into()) {
            pinocchio_log::log!(
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
            pinocchio_log::log!(
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
macro_rules! require_pda {
    ($info:expr, $seeds: expr, $program_id: expr, $is_writable: expr) => {{
        let pda = match pinocchio::Address::create_program_address($seeds, $program_id) {
            Ok(pda) => pda,
            Err(_) => {
                pinocchio_log::log!(
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
            pinocchio_log::log!(
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
            pinocchio_log::log!(
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

/// Errors if:
/// - Account is not owned by expected program.
#[inline(always)]
pub fn require_owned_pda(
    info: &AccountView,
    owner: &Address,
    label: &str,
) -> Result<(), ProgramError> {
    if !address_eq(unsafe { info.owner() }, owner) {
        log!("Invalid account owner for {}:", label);
        info.address().log();
        unsafe { info.owner() }.log();
        owner.log();
        return Err(ProgramError::InvalidAccountOwner);
    }
    Ok(())
}

/// Errors if:
/// - Account is not a signer.
#[inline(always)]
pub fn require_signer(
    info: &AccountView,
    label: &str,
) -> Result<(), ProgramError> {
    if !info.is_signer() {
        log!("Account needs to be signer {}: ", label);
        info.address().log();
        return Err(ProgramError::MissingRequiredSignature);
    }

    Ok(())
}

/// Errors if:
/// - Address does not match PDA derived from provided seeds.
#[inline(always)]
pub fn require_pda(
    info: &AccountView,
    seeds: &[&[u8]],
    program_id: &Address,
    is_writable: bool,
    label: &str,
) -> Result<u8, ProgramError> {
    let pda = Address::find_program_address(seeds, program_id);

    if !address_eq(info.address(), &pda.0) {
        log!("Invalid seeds for {}: ", label);
        info.address().log();
        return Err(ProgramError::InvalidSeeds);
    }

    if is_writable && !info.is_writable() {
        log!("Account needs to be writable. Label: {}", label);
        info.address().log();
        return Err(ProgramError::Immutable);
    }

    Ok(pda.1)
}

/// Returns true if the account is uninitialized based on the following conditions:
/// - Owner is the system program.
/// - Data is empty.
pub fn is_uninitialized_account(info: &AccountView) -> bool {
    address_eq(unsafe { info.owner() }, &pinocchio_system::ID)
        && info.is_data_empty()
}

/// Errors if:
/// - Owner is not the system program.
/// - Data is not empty.
/// - Account is not writable.
#[inline(always)]
pub fn require_uninitialized_account(
    info: &AccountView,
    is_writable: bool,
    ctx: impl RequireUninitializedAccountCtx,
) -> Result<(), ProgramError> {
    if !address_eq(unsafe { info.owner() }, &pinocchio_system::id()) {
        log!(
            "Invalid owner for account. Label: {}; account and owner: ",
            ctx.label()
        );
        info.address().log();
        unsafe { info.owner() }.log();
        return Err(ctx.invalid_account_owner());
    }

    if !info.is_data_empty() {
        log!(
            "Account needs to be uninitialized. Label: {}, account: ",
            ctx.label(),
        );
        info.address().log();
        return Err(ctx.account_already_initialized());
    }

    if is_writable && !info.is_writable() {
        log!(
            "Account needs to be writable. label: {}, account: ",
            ctx.label()
        );
        info.address().log();
        return Err(ctx.immutable());
    }

    Ok(())
}

/// Errors if:
/// - Address does not match PDA derived from provided seeds.
/// - Cannot load as an uninitialized account.
#[inline(always)]
pub fn require_uninitialized_pda(
    info: &AccountView,
    seeds: &[&[u8]],
    program_id: &Address,
    is_writable: bool,
    ctx: impl RequireUninitializedAccountCtx,
) -> Result<u8, ProgramError> {
    let pda = Address::find_program_address(seeds, program_id);

    if !address_eq(info.address(), &pda.0) {
        log!("Invalid seeds for account {}: ", ctx.label());
        info.address().log();
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
    info: &AccountView,
    seeds: &[&[u8]],
    program_id: &Address,
    is_writable: bool,
    label: &str,
) -> Result<u8, ProgramError> {
    let pda = Address::find_program_address(seeds, program_id);
    if !address_eq(info.address(), &pda.0) {
        log!("Invalid seeds (label: {}) for account ", label);
        info.address().log();
        return Err(ProgramError::InvalidSeeds);
    }

    require_owned_pda(info, program_id, label)?;

    if is_writable && !info.is_writable() {
        log!("Account needs to be writable. label: {}, account: ", label);
        info.address().log();
        return Err(ProgramError::Immutable);
    }

    Ok(pda.1)
}

/// Errors if:
/// - Address does not match the expected value.
/// - Account is not executable.
#[inline(always)]
#[allow(dead_code)]
pub fn require_program(
    info: &AccountView,
    key: &Address,
    label: &str,
) -> Result<(), ProgramError> {
    if !address_eq(info.address(), key) {
        log!("Invalid program account {}: ", label);
        info.address().log();
        return Err(ProgramError::IncorrectProgramId);
    }

    if !info.executable() {
        log!("{} program is not executable: ", label);
        info.address().log();
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(())
}

/// Load fee vault PDA
/// - Protocol fees vault PDA
pub fn require_initialized_protocol_fees_vault(
    fees_vault: &AccountView,
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
    validator: &AccountView,
    validator_fees_vault: &AccountView,
    is_writable: bool,
) -> Result<(), ProgramError> {
    let pda = validator_fees_vault_pda_from_validator(
        &validator.address().to_bytes().into(),
    )
    .to_bytes()
    .into();
    if !address_eq(validator_fees_vault.address(), &pda) {
        log!("Invalid validator fees vault PDA, expected: ");
        pda.log();
        log!("but got: ");
        validator_fees_vault.address().log();
        return Err(DlpError::InvalidAuthority.into());
    }
    require_initialized_pda(
        validator_fees_vault,
        &[pda::VALIDATOR_FEES_VAULT_TAG, validator.address().as_ref()],
        &crate::fast::ID,
        is_writable,
        "validator fees vault",
    )?;
    Ok(())
}

/// Load program config PDA
/// - Program config PDA must be initialized with the expected seeds and owner, or not exists
pub fn require_program_config(
    program_config: &AccountView,
    program: &Address,
    is_writable: bool,
) -> Result<bool, ProgramError> {
    let pda = program_config_from_program_id(&program.to_bytes().into());
    if !address_eq(&pda.to_bytes().into(), program_config.address()) {
        log!("Invalid program config PDA, expected: ");
        pda.log();
        log!("but got: ");
        program_config.address().log();
        return Err(DlpError::InvalidAuthority.into());
    }
    require_pda(
        program_config,
        &[pda::PROGRAM_CONFIG_TAG, program.as_ref()],
        &crate::fast::ID,
        is_writable,
        "program config",
    )?;
    Ok(!address_eq(
        unsafe { program_config.owner() },
        &pinocchio_system::ID,
    ))
}

/// Load initialized delegation record
/// - Delegation record must be derived from the delegated account
pub fn require_initialized_delegation_record(
    delegated_account: &AccountView,
    delegation_record: &AccountView,
    is_writable: bool,
) -> Result<(), ProgramError> {
    require_initialized_pda(
        delegation_record,
        &[
            pda::DELEGATION_RECORD_TAG,
            delegated_account.address().as_ref(),
        ],
        &crate::fast::ID,
        is_writable,
        "delegation record",
    )?;
    Ok(())
}

/// Load initialized delegation metadata
/// - Delegation metadata must be derived from the delegated account
pub fn require_initialized_delegation_metadata(
    delegated_account: &AccountView,
    delegation_metadata: &AccountView,
    is_writable: bool,
) -> Result<(), ProgramError> {
    require_initialized_pda(
        delegation_metadata,
        &[
            pda::DELEGATION_METADATA_TAG,
            delegated_account.address().as_ref(),
        ],
        &crate::fast::ID,
        is_writable,
        "delegation metadata",
    )?;
    Ok(())
}

/// Load initialized commit state account
/// - Commit state account must be derived from the delegated account pubkey
pub fn require_initialized_commit_state(
    delegated_account: &AccountView,
    commit_state: &AccountView,
    is_writable: bool,
) -> Result<(), ProgramError> {
    require_initialized_pda(
        commit_state,
        &[pda::COMMIT_STATE_TAG, delegated_account.address().as_ref()],
        &crate::fast::ID,
        is_writable,
        "commit state",
    )?;
    Ok(())
}

/// Load initialized commit state record
/// - Commit record account must be derived from the delegated account pubkey
pub fn require_initialized_commit_record(
    delegated_account: &AccountView,
    commit_record: &AccountView,
    is_writable: bool,
) -> Result<(), ProgramError> {
    require_initialized_pda(
        commit_record,
        &[pda::COMMIT_RECORD_TAG, delegated_account.address().as_ref()],
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
pub trait RequireUninitializedAccountCtx {
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
        pub struct $name;

        impl $crate::requires::RequireUninitializedAccountCtx for $name {
            fn label(&self) -> &str {
                $label
            }

            fn invalid_seeds(&self) -> pinocchio::error::ProgramError {
                $seeds.into()
            }

            fn invalid_account_owner(&self) -> pinocchio::error::ProgramError {
                $owner.into()
            }

            fn account_already_initialized(
                &self,
            ) -> pinocchio::error::ProgramError {
                $already_init.into()
            }

            fn immutable(&self) -> pinocchio::error::ProgramError {
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
    account_already_initialized =
        DlpError::DelegationMetadataAlreadyInitialized,
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
    program_data: &AccountView,
    admin: &AccountView,
) -> Result<(), ProgramError> {
    #[cfg(feature = "unit_test_config")]
    {
        let _ = program_data;

        require_eq_keys!(
            &Address::from(
                crate::consts::DEFAULT_VALIDATOR_IDENTITY.to_bytes()
            ),
            admin.address(),
            ProgramError::IncorrectAuthority
        );
        Ok(())
    }

    #[cfg(not(feature = "unit_test_config"))]
    {
        // Derive and validate program data address
        require_eq_keys!(
            &crate::consts::DELEGATION_PROGRAM_DATA_ID,
            program_data.address(),
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
        let data = program_data.try_borrow()?;
        if data.len() >= offset_of_upgrade_authority_address + 33
            && data[0] == PROGRAM_DATA
            && data[offset_of_upgrade_authority_address] == OPTION_SOME
        {
            let bytes = &data[offset_of_upgrade_authority_address + 1
                ..offset_of_upgrade_authority_address + 33];

            let upgrade_authority_address =
                unsafe { &*(bytes.as_ptr() as *const Address) };

            require_eq_keys!(
                upgrade_authority_address,
                admin.address(),
                ProgramError::IncorrectAuthority
            );

            Ok(())
        } else {
            Err(ProgramError::InvalidAccountData)
        }
    }
}
