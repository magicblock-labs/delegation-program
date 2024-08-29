use solana_program::{
    {self},
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
};
use solana_program::program_error::ProgramError;

use crate::consts::{ADMIN_PUBKEY, WHITELIST};
use crate::error::DlpError::Unauthorized;
use crate::loaders::{load_signer, load_uninitialized_pda};
use crate::utils::create_pda;

/// Process whitelisting a validator
///
/// 1. Create a whitelisting record for a validator identity
pub fn process_whitelist(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    _data: &[u8],
) -> ProgramResult {

    // Load Accounts
    let [payer, admin, validator_identify, white_list_record, system_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Check if the payer and admin are signers
    load_signer(payer)?;
    load_signer(admin)?;

    // Check if the admin is the correct one
    if !admin.key.eq(&ADMIN_PUBKEY) {
        return Err(Unauthorized.into());
    }

    let bump_white_list_record = load_uninitialized_pda(white_list_record, &[WHITELIST, validator_identify.key.as_ref()], &crate::id())?;

    // Create the whitelist record
    create_pda(
        white_list_record,
        &crate::id(),
        8,
        &[WHITELIST, validator_identify.key.as_ref(), &[bump_white_list_record]],
        system_program,
        payer,
    )?;

    Ok(())
}
