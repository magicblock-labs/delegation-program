use crate::consts::{
    COMMIT_RECORD, COMMIT_STATE, DELEGATION_METADATA, DELEGATION_RECORD, FEES_VAULT,
    PROGRAM_CONFIG, VALIDATOR_FEES_VAULT,
};
use crate::error::DlpError::InvalidAuthority;
use crate::pda::{program_config_pda_from_pubkey, validator_fees_vault_pda_from_pubkey};
use solana_program::{
    account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey, system_program,
    sysvar,
};

/// Errors if:
/// - Account is not owned by expected program.
pub fn load_owned_pda(info: &AccountInfo, owner: &Pubkey) -> Result<(), ProgramError> {
    if !info.owner.eq(owner) {
        msg!("Invalid account owner for {:?}", info.key);
        return Err(ProgramError::InvalidAccountOwner);
    }

    Ok(())
}

/// Errors if:
/// - Account is not a signer.
pub fn load_signer(info: &AccountInfo) -> Result<(), ProgramError> {
    if !info.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    Ok(())
}

/// Errors if:
/// - Address does not match PDA derived from provided seeds.
pub fn load_pda(
    info: &AccountInfo,
    seeds: &[&[u8]],
    program_id: &Pubkey,
    is_writable: bool,
) -> Result<u8, ProgramError> {
    let pda = Pubkey::find_program_address(seeds, program_id);

    if info.key.ne(&pda.0) {
        return Err(ProgramError::InvalidSeeds);
    }

    if !info.is_writable.eq(&is_writable) {
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(pda.1)
}

/// Errors if:
/// - Address does not match PDA derived from provided seeds.
/// - Cannot load as an uninitialized account.
pub fn load_uninitialized_pda(
    info: &AccountInfo,
    seeds: &[&[u8]],
    program_id: &Pubkey,
) -> Result<u8, ProgramError> {
    let pda = Pubkey::find_program_address(seeds, program_id);

    if info.key.ne(&pda.0) {
        msg!("Invalid seeds for account: {:?}", info.key);
        return Err(ProgramError::InvalidSeeds);
    }

    load_uninitialized_account(info)?;
    Ok(pda.1)
}

/// Errors if:
/// - Address does not match PDA derived from provided seeds.
/// - Owner is not the expected program.
/// - Account is not writable if set to writable.
pub fn load_initialized_pda(
    info: &AccountInfo,
    seeds: &[&[u8]],
    program_id: &Pubkey,
    is_writable: bool,
) -> Result<u8, ProgramError> {
    let pda = Pubkey::find_program_address(seeds, program_id);

    if info.key.ne(&pda.0) {
        msg!("Invalid seeds for account: {:?}", info.key);
        return Err(ProgramError::InvalidSeeds);
    }

    load_owned_pda(info, program_id)?;

    if is_writable && !info.is_writable {
        msg!("Account {:?} is not writable", info.key);
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(pda.1)
}

/// Errors if:
/// - Owner is not the system program.
/// - Data is not empty.
/// - Account is not writable.
#[allow(dead_code)]
pub fn load_uninitialized_account(info: &AccountInfo) -> Result<(), ProgramError> {
    if info.owner.ne(&system_program::id()) {
        msg!("Invalid owner for account: {:?}", info.key);
        return Err(ProgramError::InvalidAccountOwner);
    }

    if !info.data_is_empty() {
        msg!("Account {:?} is not empty", info.key);
        return Err(ProgramError::AccountAlreadyInitialized);
    }

    if !info.is_writable {
        msg!("Account {:?} is not writable", info.key);
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(())
}

/// Errors if:
/// - Owner is not the sysvar address.
/// - Account cannot load with the expected address.
#[allow(dead_code)]
pub fn load_sysvar(info: &AccountInfo, key: Pubkey) -> Result<(), ProgramError> {
    if info.owner.ne(&sysvar::id()) {
        msg!("Invalid owner for sysvar: {:?}", info.key);
        return Err(ProgramError::InvalidAccountOwner);
    }

    load_account(info, key, false)
}

/// Errors if:
/// - Address does not match the expected value.
/// - Expected to be writable, but is not.
pub fn load_account(
    info: &AccountInfo,
    key: Pubkey,
    is_writable: bool,
) -> Result<(), ProgramError> {
    if info.key.ne(&key) {
        msg!("Invalid account: {:?}", info.key);
        return Err(ProgramError::InvalidAccountData);
    }

    if is_writable && !info.is_writable {
        msg!("Account {:?} is not writable", info.key);
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(())
}

/// Errors if:
/// - Address does not match the expected value.
/// - Account is not executable.
pub fn load_program(info: &AccountInfo, key: Pubkey) -> Result<(), ProgramError> {
    if info.key.ne(&key) {
        msg!("Invalid program account: {:?}", info.key);
        return Err(ProgramError::IncorrectProgramId);
    }

    if !info.executable {
        msg!("Program is not executable: {:?}", info.key);
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(())
}

/// Load fee vault PDA
/// - Protocol fees vault PDA
pub fn load_fees_vault(fees_vault: &AccountInfo) -> Result<(), ProgramError> {
    load_initialized_pda(fees_vault, &[FEES_VAULT], &crate::id(), true)?;
    Ok(())
}

/// Load validator fee vault PDA
/// - Validator fees vault PDA must be derived from the validator pubkey
/// - Validator fees vault PDA must be initialized with the expected seeds and owner
pub fn load_validator_fees_vault(
    validator: &AccountInfo,
    validator_fees_vault: &AccountInfo,
) -> Result<(), ProgramError> {
    if !validator_fees_vault_pda_from_pubkey(validator.key).eq(validator_fees_vault.key) {
        return Err(InvalidAuthority.into());
    }
    load_initialized_pda(
        validator_fees_vault,
        &[VALIDATOR_FEES_VAULT, &validator.key.to_bytes()],
        &crate::id(),
        true,
    )?;
    Ok(())
}

/// Load program config PDA
/// - Program config PDA must be initialized with the expected seeds and owner, or not exists
pub fn load_program_config(
    program_config: &AccountInfo,
    program: Pubkey,
) -> Result<bool, ProgramError> {
    if !program_config_pda_from_pubkey(&program).eq(program_config.key) {
        return Err(InvalidAuthority.into());
    }
    load_pda(
        program_config,
        &[PROGRAM_CONFIG, program.as_ref()],
        &crate::id(),
        false,
    )?;
    Ok(!program_config.owner.eq(&system_program::ID))
}

/// Load initialized delegation record
/// - Delegation record must be derived from the delegated account
pub fn load_initialized_delegation_record(
    delegated_account: &AccountInfo,
    delegation_record: &AccountInfo,
) -> Result<(), ProgramError> {
    load_initialized_pda(
        delegation_record,
        &[DELEGATION_RECORD, &delegated_account.key.to_bytes()],
        &crate::id(),
        true,
    )?;
    Ok(())
}

/// Load initialized delegation metadata
/// - Delegation metadata must be derived from the delegated account
pub fn load_initialized_delegation_metadata(
    delegated_account: &AccountInfo,
    delegation_metadata: &AccountInfo,
) -> Result<(), ProgramError> {
    load_initialized_pda(
        delegation_metadata,
        &[DELEGATION_METADATA, &delegated_account.key.to_bytes()],
        &crate::id(),
        true,
    )?;
    Ok(())
}

/// Load initialized commit state account
/// - Commit state account must be derived from the delegated account pubkey
pub fn load_initialized_commit_state(
    delegated_account: &AccountInfo,
    commit_state: &AccountInfo,
) -> Result<(), ProgramError> {
    load_initialized_pda(
        commit_state,
        &[COMMIT_STATE, &delegated_account.key.to_bytes()],
        &crate::id(),
        true,
    )?;
    Ok(())
}

/// Load initialized commit state record
/// - Commit record account must be derived from the delegated account pubkey
pub fn load_initialized_commit_record(
    delegated_account: &AccountInfo,
    commit_record: &AccountInfo,
) -> Result<(), ProgramError> {
    load_initialized_pda(
        commit_record,
        &[COMMIT_RECORD, &delegated_account.key.to_bytes()],
        &crate::id(),
        true,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use solana_program::{account_info::AccountInfo, pubkey::Pubkey, system_program};

    use crate::utils::loaders::{
        load_account, load_signer, load_sysvar, load_uninitialized_account,
    };

    use super::load_program;

    #[test]
    pub fn test_signer_not_signer() {
        let key = Pubkey::new_unique();
        let mut lamports = 1_000_000_000;
        let mut data = [];
        let owner = system_program::id();
        let info = AccountInfo::new(
            &key,
            false,
            false,
            &mut lamports,
            &mut data,
            &owner,
            false,
            0,
        );
        assert!(load_signer(&info).is_err());
    }

    #[test]
    pub fn test_load_uninitialized_account_bad_owner() {
        let key = Pubkey::new_unique();
        let mut lamports = 1_000_000_000;
        let mut data = [];
        let owner = crate::id();
        let info = AccountInfo::new(
            &key,
            false,
            true,
            &mut lamports,
            &mut data,
            &owner,
            false,
            0,
        );
        assert!(load_uninitialized_account(&info).is_err());
    }

    #[test]
    pub fn test_load_uninitialized_account_data_not_empty() {
        let key = Pubkey::new_unique();
        let mut lamports = 1_000_000_000;
        let mut data = [0];
        let owner = system_program::id();
        let info = AccountInfo::new(
            &key,
            false,
            true,
            &mut lamports,
            &mut data,
            &owner,
            false,
            0,
        );
        assert!(load_uninitialized_account(&info).is_err());
    }

    #[test]
    pub fn test_load_uninitialized_account_not_writeable() {
        let key = Pubkey::new_unique();
        let mut lamports = 1_000_000_000;
        let mut data = [];
        let owner = system_program::id();
        let info = AccountInfo::new(
            &key,
            false,
            false,
            &mut lamports,
            &mut data,
            &owner,
            false,
            0,
        );
        assert!(load_uninitialized_account(&info).is_err());
    }

    #[test]
    pub fn test_load_sysvar_bad_owner() {
        let key = Pubkey::new_unique();
        let mut lamports = 1_000_000_000;
        let mut data = [];
        let owner = system_program::id();
        let info = AccountInfo::new(
            &key,
            false,
            false,
            &mut lamports,
            &mut data,
            &owner,
            false,
            0,
        );
        assert!(load_sysvar(&info, key).is_err());
    }

    #[test]
    pub fn test_load_account_bad_key() {
        let key = Pubkey::new_unique();
        let mut lamports = 1_000_000_000;
        let mut data = [];
        let owner = system_program::id();
        let info = AccountInfo::new(
            &key,
            false,
            false,
            &mut lamports,
            &mut data,
            &owner,
            false,
            0,
        );
        assert!(load_account(&info, Pubkey::new_unique(), false).is_err());
    }

    #[test]
    pub fn test_load_account_not_writeable() {
        let key = Pubkey::new_unique();
        let mut lamports = 1_000_000_000;
        let mut data = [];
        let owner = system_program::id();
        let info = AccountInfo::new(
            &key,
            false,
            false,
            &mut lamports,
            &mut data,
            &owner,
            false,
            0,
        );
        assert!(load_account(&info, key, true).is_err());
    }

    #[test]
    pub fn test_load_program_bad_key() {
        let key = Pubkey::new_unique();
        let mut lamports = 1_000_000_000;
        let mut data = [];
        let owner = system_program::id();
        let info = AccountInfo::new(
            &key,
            false,
            false,
            &mut lamports,
            &mut data,
            &owner,
            true,
            0,
        );
        assert!(load_program(&info, Pubkey::new_unique()).is_err());
    }

    #[test]
    pub fn test_load_program_not_executable() {
        let key = Pubkey::new_unique();
        let mut lamports = 1_000_000_000;
        let mut data = [];
        let owner = system_program::id();
        let info = AccountInfo::new(
            &key,
            false,
            false,
            &mut lamports,
            &mut data,
            &owner,
            false,
            0,
        );
        assert!(load_program(&info, key).is_err());
    }
}
