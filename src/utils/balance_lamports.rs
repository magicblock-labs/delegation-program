use solana_program::account_info::AccountInfo;
use solana_program::program_error::ProgramError;

/// Settle the committed lamports to the delegated account
pub(crate) fn settle_lamports_balance<'a, 'info>(
    target_account: &'a AccountInfo<'info>,
    commited_state_account: &'a AccountInfo<'info>,
    lamports_difference: i64,
    validator_fees_vault: &'a AccountInfo<'info>,
) -> Result<(), ProgramError> {
    let (source, destination, amount) = match lamports_difference.cmp(&0) {
        std::cmp::Ordering::Greater => (
            target_account,
            validator_fees_vault,
            lamports_difference.unsigned_abs(),
        ),
        std::cmp::Ordering::Less => (
            commited_state_account,
            target_account,
            lamports_difference.unsigned_abs(),
        ),
        std::cmp::Ordering::Equal => return Ok(()),
    };

    **source.try_borrow_mut_lamports()? = source
        .lamports()
        .checked_sub(amount)
        .ok_or(ProgramError::InvalidAccountData)?;

    **destination.try_borrow_mut_lamports()? = destination
        .lamports()
        .checked_add(amount)
        .ok_or(ProgramError::InvalidAccountData)?;

    Ok(())
}
