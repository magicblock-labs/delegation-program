use solana_program::{
    account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey, system_program, sysvar,
};

/// Errors if:
/// - Account is not a signer.
pub fn load_signer<'a, 'info>(info: &'a AccountInfo<'info>) -> Result<(), ProgramError> {
    if !info.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    Ok(())
}

/// Errors if:
/// - Address does not match PDA derived from provided seeds.
/// - Cannot load as an uninitialized account.
pub fn load_uninitialized_pda<'a, 'info>(
    info: &'a AccountInfo<'info>,
    seeds: &[&[u8]],
    bump: u8,
    program_id: &Pubkey,
) -> Result<(), ProgramError> {
    let pda = Pubkey::find_program_address(seeds, program_id);

    if info.key.ne(&pda.0) {
        return Err(ProgramError::InvalidSeeds);
    }

    if bump.ne(&pda.1) {
        return Err(ProgramError::InvalidSeeds);
    }

    load_uninitialized_account(info)
}

/// Errors if:
/// - Owner is not the system program.
/// - Data is not empty.
/// - Account is not writable.
pub fn load_uninitialized_account<'a, 'info>(
    info: &'a AccountInfo<'info>,
) -> Result<(), ProgramError> {
    if info.owner.ne(&system_program::id()) {
        return Err(ProgramError::InvalidAccountOwner);
    }

    if !info.data_is_empty() {
        return Err(ProgramError::AccountAlreadyInitialized);
    }

    if !info.is_writable {
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(())
}

/// Errors if:
/// - Owner is not the sysvar address.
/// - Account cannot load with the expected address.
pub fn load_sysvar<'a, 'info>(
    info: &'a AccountInfo<'info>,
    key: Pubkey,
) -> Result<(), ProgramError> {
    if info.owner.ne(&sysvar::id()) {
        return Err(ProgramError::InvalidAccountOwner);
    }

    load_account(info, key, false)
}

/// Errors if:
/// - Address does not match the expected value.
/// - Expected to be writable, but is not.
pub fn load_account<'a, 'info>(
    info: &'a AccountInfo<'info>,
    key: Pubkey,
    is_writable: bool,
) -> Result<(), ProgramError> {
    if info.key.ne(&key) {
        return Err(ProgramError::InvalidAccountData);
    }

    if is_writable && !info.is_writable {
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(())
}

/// Errors if:
/// - Address does not match the expected value.
/// - Account is not executable.
pub fn load_program<'a, 'info>(
    info: &'a AccountInfo<'info>,
    key: Pubkey,
) -> Result<(), ProgramError> {
    if info.key.ne(&key) {
        return Err(ProgramError::IncorrectProgramId);
    }

    if !info.executable {
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use solana_program::{
        account_info::AccountInfo, keccak::Hash as KeccakHash, program_option::COption,
        program_pack::Pack, pubkey::Pubkey, system_program,
    };
    use spl_token::state::{AccountState, Mint};

    use crate::{
        loaders::{
            load_account, load_signer, load_sysvar, load_uninitialized_account,
            load_uninitialized_pda,
        },
        utils::Discriminator,
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
        let owner = spl_token::id();
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
