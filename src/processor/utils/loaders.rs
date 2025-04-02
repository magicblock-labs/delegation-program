use crate::error::DlpError::InvalidAuthority;
use crate::pda::{program_config_from_program_id, validator_fees_vault_pda_from_validator};
use crate::{
    commit_record_seeds_from_delegated_account, commit_state_seeds_from_delegated_account,
    delegation_metadata_seeds_from_delegated_account,
    delegation_record_seeds_from_delegated_account, fees_vault_seeds,
    program_config_seeds_from_program_id, validator_fees_vault_seeds_from_validator,
};
use solana_program::bpf_loader_upgradeable::UpgradeableLoaderState;
use solana_program::{
    account_info::AccountInfo, bpf_loader_upgradeable, msg, program_error::ProgramError,
    pubkey::Pubkey, system_program, sysvar,
};

/// Errors if:
/// - Account is not owned by expected program.
pub fn load_owned_pda(info: &AccountInfo, owner: &Pubkey, label: &str) -> Result<(), ProgramError> {
    if !info.owner.eq(owner) {
        msg!("Invalid account owner for {} ({})", label, info.key);
        return Err(ProgramError::InvalidAccountOwner);
    }

    Ok(())
}

/// Errors if:
/// - Account is not a signer.
pub fn load_signer(info: &AccountInfo, label: &str) -> Result<(), ProgramError> {
    if !info.is_signer {
        msg!("Account needs to be signer {} ({})", label, info.key);
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
    label: &str,
) -> Result<u8, ProgramError> {
    let pda = Pubkey::find_program_address(seeds, program_id);

    if info.key.ne(&pda.0) {
        msg!("Invalid seeds for {} ({})", label, info.key);
        return Err(ProgramError::InvalidSeeds);
    }

    if !info.is_writable.eq(&is_writable) {
        msg!("Account {} ({}) needs to be writable", label, info.key);
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
    is_writable: bool,
    label: &str,
) -> Result<u8, ProgramError> {
    let pda = Pubkey::find_program_address(seeds, program_id);

    if info.key.ne(&pda.0) {
        msg!("Invalid seeds for account: {} ({})", label, info.key);
        return Err(ProgramError::InvalidSeeds);
    }

    load_uninitialized_account(info, is_writable, label)?;
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
    label: &str,
) -> Result<u8, ProgramError> {
    let pda = Pubkey::find_program_address(seeds, program_id);

    if info.key.ne(&pda.0) {
        msg!("Invalid seeds for account: {}", info.key);
        return Err(ProgramError::InvalidSeeds);
    }

    load_owned_pda(info, program_id, label)?;

    if is_writable && !info.is_writable {
        msg!("Account {} is not writable", info.key);
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(pda.1)
}

/// Returns true if the account is uninitialized based on the following conditions:
/// - Owner is the system program.
/// - Data is empty.
pub fn is_uninitialized_account(info: &AccountInfo) -> bool {
    info.owner.eq(&system_program::id()) && info.data_is_empty()
}

/// Errors if:
/// - Owner is not the system program.
/// - Data is not empty.
/// - Account is not writable.
#[allow(dead_code)]
pub fn load_uninitialized_account(
    info: &AccountInfo,
    is_writable: bool,
    label: &str,
) -> Result<(), ProgramError> {
    if info.owner.ne(&system_program::id()) {
        msg!("Invalid owner for account: {} ({})", label, info.key);
        return Err(ProgramError::InvalidAccountOwner);
    }

    if !info.data_is_empty() {
        msg!("Account {} ({}) needs to be uninitialized", label, info.key);
        return Err(ProgramError::AccountAlreadyInitialized);
    }

    if is_writable && !info.is_writable {
        msg!("Account {} ({}) needs to be writable", label, info.key);
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
        msg!("Invalid owner for sysvar: {}", info.key);
        return Err(ProgramError::InvalidAccountOwner);
    }

    load_account(info, key, false, "sysvar")
}

/// Errors if:
/// - Address does not match the expected value.
/// - Expected to be writable, but is not.
pub fn load_account(
    info: &AccountInfo,
    key: Pubkey,
    is_writable: bool,
    label: &str,
) -> Result<(), ProgramError> {
    if info.key.ne(&key) {
        msg!("Expected key {} for {}, but got {}", key, label, info.key);
        return Err(ProgramError::InvalidAccountData);
    }

    if is_writable && !info.is_writable {
        msg!("Account {} ({}) needs to be writable", label, info.key);
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(())
}

/// Errors if:
/// - Address does not match the expected value.
/// - Account is not executable.
pub fn load_program(info: &AccountInfo, key: Pubkey, label: &str) -> Result<(), ProgramError> {
    if info.key.ne(&key) {
        msg!("Invalid program account: {} ({})", label, info.key);
        return Err(ProgramError::IncorrectProgramId);
    }

    if !info.executable {
        msg!("{} program is not executable: {}", label, info.key);
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(())
}

/// Get the program upgrade authority for a given program
pub fn load_program_upgrade_authority(
    program: &Pubkey,
    program_data: &AccountInfo,
) -> Result<Option<Pubkey>, ProgramError> {
    let program_data_address =
        Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::id()).0;

    // During tests, the upgrade authority is a test pubkey
    #[cfg(feature = "unit_test_config")]
    if program.eq(&crate::ID) {
        return Ok(Some(solana_program::pubkey!(
            "tEsT3eV6RFCWs1BZ7AXTzasHqTtMnMLCB2tjQ42TDXD"
        )));
    }

    if !program_data_address.eq(program_data.key) {
        msg!(
            "Expected program data address to be {}, but got {}",
            program_data_address,
            program_data.key
        );
        return Err(ProgramError::InvalidAccountData);
    }

    let program_account_data = program_data.try_borrow_data()?;
    if let UpgradeableLoaderState::ProgramData {
        upgrade_authority_address,
        ..
    } = bincode::deserialize(&program_account_data).map_err(|_| {
        msg!("Unable to deserialize ProgramData {}", program);
        ProgramError::InvalidAccountData
    })? {
        Ok(upgrade_authority_address)
    } else {
        msg!("Expected program account {} to hold ProgramData", program);
        Err(ProgramError::InvalidAccountData)
    }
}

/// Load fee vault PDA
/// - Protocol fees vault PDA
pub fn load_initialized_protocol_fees_vault(
    fees_vault: &AccountInfo,
    is_writable: bool,
) -> Result<(), ProgramError> {
    load_initialized_pda(
        fees_vault,
        fees_vault_seeds!(),
        &crate::id(),
        is_writable,
        "protocol fees vault",
    )?;
    Ok(())
}

/// Load validator fee vault PDA
/// - Validator fees vault PDA must be derived from the validator pubkey
/// - Validator fees vault PDA must be initialized with the expected seeds and owner
pub fn load_initialized_validator_fees_vault(
    validator: &AccountInfo,
    validator_fees_vault: &AccountInfo,
    is_writable: bool,
) -> Result<(), ProgramError> {
    let pda = validator_fees_vault_pda_from_validator(validator.key);
    if !pda.eq(validator_fees_vault.key) {
        msg!(
            "Invalid validator fees vault PDA, expected {} but got {}",
            pda,
            validator_fees_vault.key
        );
        return Err(InvalidAuthority.into());
    }
    load_initialized_pda(
        validator_fees_vault,
        validator_fees_vault_seeds_from_validator!(validator.key),
        &crate::id(),
        is_writable,
        "validator fees vault",
    )?;
    Ok(())
}

/// Load program config PDA
/// - Program config PDA must be initialized with the expected seeds and owner, or not exists
pub fn load_program_config(
    program_config: &AccountInfo,
    program: Pubkey,
    is_writable: bool,
) -> Result<bool, ProgramError> {
    let pda = program_config_from_program_id(&program);
    if !pda.eq(program_config.key) {
        msg!(
            "Invalid program config PDA, expected {} but got {}",
            pda,
            program_config.key
        );
        return Err(InvalidAuthority.into());
    }
    load_pda(
        program_config,
        program_config_seeds_from_program_id!(program),
        &crate::id(),
        is_writable,
        "program config",
    )?;
    Ok(!program_config.owner.eq(&system_program::ID))
}

/// Load initialized delegation record
/// - Delegation record must be derived from the delegated account
pub fn load_initialized_delegation_record(
    delegated_account: &AccountInfo,
    delegation_record: &AccountInfo,
    is_writable: bool,
) -> Result<(), ProgramError> {
    load_initialized_pda(
        delegation_record,
        delegation_record_seeds_from_delegated_account!(delegated_account.key),
        &crate::id(),
        is_writable,
        "delegation record",
    )?;
    Ok(())
}

/// Load initialized delegation metadata
/// - Delegation metadata must be derived from the delegated account
pub fn load_initialized_delegation_metadata(
    delegated_account: &AccountInfo,
    delegation_metadata: &AccountInfo,
    is_writable: bool,
) -> Result<(), ProgramError> {
    load_initialized_pda(
        delegation_metadata,
        delegation_metadata_seeds_from_delegated_account!(delegated_account.key),
        &crate::id(),
        is_writable,
        "delegation metadata",
    )?;
    Ok(())
}

/// Load initialized commit state account
/// - Commit state account must be derived from the delegated account pubkey
pub fn load_initialized_commit_state(
    delegated_account: &AccountInfo,
    commit_state: &AccountInfo,
    is_writable: bool,
) -> Result<(), ProgramError> {
    load_initialized_pda(
        commit_state,
        commit_state_seeds_from_delegated_account!(delegated_account.key),
        &crate::id(),
        is_writable,
        "commit state",
    )?;
    Ok(())
}

/// Load initialized commit state record
/// - Commit record account must be derived from the delegated account pubkey
pub fn load_initialized_commit_record(
    delegated_account: &AccountInfo,
    commit_record: &AccountInfo,
    is_writable: bool,
) -> Result<(), ProgramError> {
    load_initialized_pda(
        commit_record,
        commit_record_seeds_from_delegated_account!(delegated_account.key),
        &crate::id(),
        is_writable,
        "commit record",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use solana_program::{account_info::AccountInfo, pubkey::Pubkey, system_program};

    use crate::processor::utils::loaders::{
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
        assert!(load_signer(&info, "not signer").is_err());
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
        assert!(load_uninitialized_account(&info, true, "bad owner").is_err());
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
        assert!(load_uninitialized_account(&info, true, "data not empty").is_err());
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
        assert!(load_uninitialized_account(&info, true, "not writeable").is_err());
    }

    #[test]
    pub fn test_load_uninitialized_account_not_writeable_on_purpose() {
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
        assert!(load_uninitialized_account(&info, false, "not writable").is_ok());
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
        assert!(load_account(&info, Pubkey::new_unique(), false, "bad key").is_err());
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
        assert!(load_account(&info, key, true, "not writeable").is_err());
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
        assert!(load_program(&info, Pubkey::new_unique(), "bad key").is_err());
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
        assert!(load_program(&info, key, "not executable").is_err());
    }
}
