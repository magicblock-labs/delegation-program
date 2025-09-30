use pinocchio::account_info::AccountInfo;
use pinocchio::instruction::Signer;
use pinocchio::program_error::ProgramError;
use pinocchio::pubkey::Pubkey;
use pinocchio::sysvars::rent::Rent;
use pinocchio::sysvars::Sysvar;
use pinocchio::ProgramResult;
use pinocchio_system::instructions as system;

/// Creates a new pda
#[inline(always)]
pub(crate) fn create_pda(
    target_account: &AccountInfo,
    owner: &Pubkey,
    space: usize,
    pda_signers: &[Signer],
    payer: &AccountInfo,
) -> ProgramResult {
    // Create the account manually or using the create instruction

    let rent = Rent::get()?;
    if target_account.lamports().eq(&0) {
        // If balance is zero, create account
        system::CreateAccount {
            from: payer,
            to: target_account,
            lamports: rent.minimum_balance(space),
            space: space as u64,
            owner,
        }
        .invoke_signed(pda_signers)
    } else {
        // Otherwise, if balance is nonzero:

        // 1) transfer sufficient lamports for rent exemption
        let rent_exempt_balance = rent
            .minimum_balance(space)
            .saturating_sub(target_account.lamports());
        if rent_exempt_balance > 0 {
            system::Transfer {
                from: payer,
                to: target_account,
                lamports: rent_exempt_balance,
            }
            .invoke()?;
        }

        // 2) allocate space for the account
        system::Allocate {
            account: target_account,
            space: space as u64,
        }
        .invoke_signed(pda_signers)?;

        // 3) assign our program as the owner
        system::Assign {
            account: target_account,
            owner,
        }
        .invoke_signed(pda_signers)
    }
}

/// Close PDA
#[inline(always)]
pub(crate) fn close_pda(target_account: &AccountInfo, destination: &AccountInfo) -> ProgramResult {
    // Transfer tokens from the account to the destination.
    unsafe {
        *destination.borrow_mut_lamports_unchecked() = destination
            .lamports()
            .checked_add(target_account.lamports())
            .ok_or(ProgramError::ArithmeticOverflow)?;

        *target_account.borrow_mut_lamports_unchecked() = 0;

        target_account.assign(&pinocchio_system::ID);
    }

    target_account.resize(0).map_err(Into::into)
}

/// Close PDA with fees, distributing the fees to the specified addresses in sequence
/// The total fees are calculated as `fee_percentage` of the total lamports in the PDA
/// Each fee address receives fee_percentage % of the previous fee address's amount
pub(crate) fn close_pda_with_fees(
    target_account: &AccountInfo,
    destination: &AccountInfo,
    fees_addresses: &[&AccountInfo],
    fee_percentage: u8,
) -> ProgramResult {
    if fees_addresses.is_empty() || fee_percentage > 100 {
        return Err(ProgramError::InvalidArgument);
    }

    let init_lamports = target_account.lamports();
    let total_fee_amount = target_account
        .lamports()
        .checked_mul(fee_percentage as u64)
        .and_then(|v| v.checked_div(100))
        .ok_or(ProgramError::InsufficientFunds)?;

    let mut fees: Vec<u64> = vec![total_fee_amount; fees_addresses.len()];

    let mut fee_amount = total_fee_amount;
    for fee in fees.iter_mut().take(fees_addresses.len()).skip(1) {
        fee_amount = fee_amount
            .checked_mul(fee_percentage as u64)
            .and_then(|v| v.checked_div(100))
            .ok_or(ProgramError::InsufficientFunds)?;
        *fee = fee_amount;
    }

    for i in 0..fees.len() - 1 {
        fees[i] -= fees[i + 1];
    }

    for (i, &fee_address) in fees_addresses.iter().enumerate() {
        unsafe {
            *fee_address.borrow_mut_lamports_unchecked() = fee_address
                .lamports()
                .checked_add(fees[i])
                .ok_or(ProgramError::InsufficientFunds)?;
        }
    }

    unsafe {
        *destination.borrow_mut_lamports_unchecked() = destination
            .lamports()
            .checked_add(init_lamports - total_fee_amount)
            .ok_or(ProgramError::InsufficientFunds)?;

        *target_account.borrow_mut_lamports_unchecked() = 0;

        target_account.assign(&pinocchio_system::ID);
    }
    target_account.resize(0).map_err(Into::into)
}
