use crate::args::WhitelistValidatorForProgramArgs;
use crate::consts::{ADMIN_PUBKEY, PROGRAM_CONFIG};
use crate::error::DlpError::Unauthorized;
use crate::processor::utils::loaders::{load_pda, load_program, load_signer};
use crate::processor::utils::pda::{create_pda, resize_pda};
use crate::state::ProgramConfig;
use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::bpf_loader_upgradeable::UpgradeableLoaderState;
use solana_program::program_error::ProgramError;
use solana_program::{
    account_info::AccountInfo,
    bpf_loader_upgradeable,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    system_program, {self},
};

/// Whitelist a validator for a program
///
pub fn process_whitelist_validator_for_program(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let args = WhitelistValidatorForProgramArgs::try_from_slice(data)?;

    // Load Accounts
    let [authority, validator_identity, program, program_data, program_config_account, system_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    load_signer(authority)?;
    validate_authority(authority, program, program_data)?;
    load_program(system_program, system_program::id())?;

    let program_config_bump = load_pda(
        program_config_account,
        &[PROGRAM_CONFIG, program.key.as_ref()],
        &crate::id(),
        true,
    )?;

    // Get the program config. If the account doesn't exist, create it
    let mut program_config = if program_config_account.owner.eq(system_program.key) {
        create_pda(
            program_config_account,
            &crate::id(),
            8 + ProgramConfig::default().serialized_len(),
            &[PROGRAM_CONFIG, program.key.as_ref(), &[program_config_bump]],
            system_program,
            authority,
        )?;
        ProgramConfig::default()
    } else {
        let data = program_config_account.try_borrow_data()?;
        ProgramConfig::try_from_slice(&data)?
    };
    if args.insert {
        program_config
            .approved_validators
            .insert(*validator_identity.key);
    } else {
        program_config
            .approved_validators
            .remove(validator_identity.key);
    }
    resize_pda(
        authority,
        program_config_account,
        system_program,
        8 + program_config.serialized_len(),
    )?;
    let mut data = program_config_account.try_borrow_mut_data()?;
    program_config.serialize(&mut &mut data.as_mut())?;

    Ok(())
}

/// Authority is valid if either the authority is the ADMIN_PUBKEY or the program upgrade authority
fn validate_authority(
    authority: &AccountInfo,
    program: &AccountInfo,
    program_data: &AccountInfo,
) -> Result<(), ProgramError> {
    if authority.key.eq(&ADMIN_PUBKEY)
        || authority
            .key
            .eq(&get_program_upgrade_authority(program, program_data)?.ok_or(Unauthorized)?)
    {
        Ok(())
    } else {
        Err(Unauthorized.into())
    }
}

/// Get the program upgrade authority for a given program
fn get_program_upgrade_authority(
    program: &AccountInfo,
    program_data: &AccountInfo,
) -> Result<Option<Pubkey>, ProgramError> {
    let program_data_address =
        Pubkey::find_program_address(&[program.key.as_ref()], &bpf_loader_upgradeable::id()).0;

    if !program_data_address.eq(program_data.key) {
        return Err(ProgramError::InvalidAccountData);
    }

    let program_account_data = program_data.try_borrow_data()?;
    if let UpgradeableLoaderState::ProgramData {
        upgrade_authority_address,
        ..
    } =
        bincode::deserialize(&program_account_data).map_err(|_| ProgramError::InvalidAccountData)?
    {
        Ok(upgrade_authority_address)
    } else {
        Err(ProgramError::InvalidAccountData)
    }
}
