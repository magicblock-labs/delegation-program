use crate::instruction_builder::undelegate;
use solana_program::msg;
use solana_program::program::invoke;
use solana_program::program_error::ProgramError;
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey, system_program,
};

/// Undelegate ephemeral balance
///
/// Accounts:
///
///  0: `[signer]`   the validator account
///  1: `[writable]` the delegated account
///  2: `[]`         the owner program of the delegated account
///  3: `[writable]` the undelegate buffer PDA we use to store the data temporarily
///  4: `[]`         the commit state PDA
///  5: `[]`         the commit record PDA
///  6: `[writable]` the delegation record PDA
///  7: `[writable]` the delegation metadata PDA
///  8: `[]`         the rent reimbursement account
///  9: `[writable]` the protocol fees vault account
/// 10: `[writable]` the validator fees vault account
/// 11: `[]`         the system program
///
/// Requirements:
///
/// - delegated account is owned by delegation program
/// - delegation record is initialized
/// - delegation metadata is initialized
/// - protocol fees vault is initialized
/// - validator fees vault is initialized
/// - commit state is uninitialized
/// - commit record is uninitialized
/// - delegated account is NOT undelegatable
/// - owner program account matches the owner in the delegation record
/// - rent reimbursement account matches the rent payer in the delegation metadata
///
/// Steps:
///
/// - Undelegate using CPI into [`crate::processor::undelegate`]
/// - Assigns ownership back to system program
pub fn process_undelegate_ephemeral_balance(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    _data: &[u8],
) -> ProgramResult {
    let [validator, delegated_account, owner_program, _, _, _, _, _, rent_reimbursement, _, _, _] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if owner_program.key != &crate::ID {
        msg!(
            "Unexpected owner program. expected dlp, got: {}",
            owner_program.key
        );
        return Err(ProgramError::IncorrectProgramId);
    }

    // Propagate to undelegate which also runs all necessary checks.
    let undelegate_ix = undelegate(
        *validator.key,
        *delegated_account.key,
        *owner_program.key,
        *rent_reimbursement.key,
    );
    invoke(&undelegate_ix, accounts)?;

    // Assign ownership back to system_program
    delegated_account.assign(&system_program::ID);
    Ok(())
}
