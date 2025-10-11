use pinocchio::account_info::AccountInfo;
use pinocchio::program_error::ProgramError;
use pinocchio::pubkey::{self, pubkey_eq, Pubkey};
use pinocchio_log::log;

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

    if !info.is_writable().eq(&is_writable) {
        // TODO (snawaz): misleading msg
        // also better use more granular error here ProgramError::InvalidPermission
        log!("Account needs to be writable. Label: {}", label);
        pubkey::log(info.key());
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(pda.1)
}

/// Errors if:
/// - Owner is not the system program.
/// - Data is not empty.
/// - Account is not writable.
#[inline(always)]
pub fn require_uninitialized_account(
    info: &AccountInfo,
    is_writable: bool,
    label: &str,
) -> Result<(), ProgramError> {
    if !pubkey_eq(info.owner(), &pinocchio_system::id()) {
        log!(
            "Invalid owner for account. Label: {}; account and owner: ",
            label
        );
        pubkey::log(info.key());
        pubkey::log(info.owner());
        return Err(ProgramError::InvalidAccountOwner);
    }

    if !info.data_is_empty() {
        log!(
            "Account needs to be uninitialized. Label: {}, account: ",
            label,
        );
        pubkey::log(info.key());
        return Err(ProgramError::AccountAlreadyInitialized);
    }

    if is_writable && !info.is_writable() {
        log!("Account needs to be writable. label: {}, account: ", label);
        pubkey::log(info.key());
        return Err(ProgramError::InvalidAccountData);
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
    label: &str,
) -> Result<u8, ProgramError> {
    let pda = pubkey::find_program_address(seeds, program_id);

    if !pubkey_eq(info.key(), &pda.0) {
        log!("Invalid seeds for account {}: ", label);
        pubkey::log(info.key());
        return Err(ProgramError::InvalidSeeds);
    }

    require_uninitialized_account(info, is_writable, label)?;
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
