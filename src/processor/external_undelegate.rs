use crate::processor::utils::loaders::load_uninitialized_pda;
use crate::processor::utils::pda::create_pda;
use crate::state::discriminator::AccountDiscriminator;
use borsh::BorshDeserialize;
use solana_program::account_info::AccountInfo;
use solana_program::entrypoint::ProgramResult;
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;
use solana_program::{msg, system_program};

/// External undelegate implementation for accounts owned by dlp
///
/// Accounts:
///
///  0: `[writable]` the delegated account
///  1: `[writable]` the undelegated buffer account
///  2: `[signer]`   the payer account
///  3: `[]`         the system program
///
/// Requirements:
///
/// - delegated account is uninitialized
/// - undelegated buffer account is signer and owned by dlp
/// - payer account is validator
/// Steps:
///
/// - Check if dlp initiated call
/// - Check if delegated account is uninitialized
/// - Extract account discriminator
/// - Run discriminator specific actions
/// - For ephemeral balance transfer ownership to system program, zero buffer account.
pub fn process_external_undelegate(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let [delegated_account, undelegate_buffer_account, payer, system_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Verify that buffer owned by dlp
    if undelegate_buffer_account.owner != &crate::ID {
        msg!(
            "dlp program must be an owner of buffer account. actual owner: {}",
            undelegate_buffer_account.owner
        );
        return Err(ProgramError::InvalidAccountOwner);
    }
    // Verify that only dlp could be initiator of this call
    // buffer derived from dlp::ID, hence only dlp could be signer
    if !undelegate_buffer_account.is_signer {
        msg!("buffer account must be a signer!");
        return Err(ProgramError::MissingRequiredSignature);
    };

    // Check that delegated account is uninitialized and derived from delegation program
    let delegated_account_seeds: Vec<Vec<u8>> = Vec::<Vec<u8>>::try_from_slice(data)?;
    let delegated_account_seeds: Vec<&[u8]> = delegated_account_seeds
        .iter()
        .map(|v| v.as_slice())
        .collect();
    let delegated_account_bump = load_uninitialized_pda(
        delegated_account,
        &delegated_account_seeds,
        &crate::id(),
        true,
        "undelegate buffer",
    )?;

    // Re-create the original PDA
    msg!(
        "ndelegate_buffer_account.data_len(): {}",
        undelegate_buffer_account.data_len()
    );
    let discriminator: [u8; 8] = undelegate_buffer_account
        .try_borrow_data()?
        .as_ref()
        .try_into()
        .map_err(|_| ProgramError::InvalidAccountData)?;

    if discriminator == AccountDiscriminator::EphemeralBalance.to_bytes() {
        // zero data. Needed because of check in undelegate.rs:255 that checks data consistency
        // for system transfer account can't contain any data, hence length set to 0
        undelegate_buffer_account.realloc(0, false)?;
        create_pda(
            delegated_account,
            &system_program::ID,
            0,
            &delegated_account_seeds,
            delegated_account_bump,
            system_program,
            payer,
        )
    } else {
        Err(ProgramError::InvalidAccountData)
    }
}
