use pinocchio::account_info::AccountInfo;
use pinocchio::instruction::Signer;
use pinocchio::program_error::ProgramError;
use pinocchio::pubkey::Pubkey;
use pinocchio::sysvars::rent::Rent;
use pinocchio::sysvars::Sysvar;
use pinocchio::ProgramResult;
use pinocchio_system::instructions as system;

use crate::consts::PROTOCOL_FEES_PERCENTAGE;

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
    rent_reimbursement: &AccountInfo,
    fees_vault: &AccountInfo,
    validator_fees_vault: &AccountInfo,
    fee_remaining: &mut u64,
) -> ProgramResult {
    let mut destination_amount = target_account.lamports();

    if *fee_remaining > 0 && destination_amount > 0 {
        let fee_taken = (*fee_remaining).min(destination_amount);
        destination_amount -= fee_taken;
        *fee_remaining -= fee_taken;

        let protocol_fee = fee_taken
            .checked_mul(PROTOCOL_FEES_PERCENTAGE as u64)
            .and_then(|value| value.checked_div(100))
            .ok_or(ProgramError::ArithmeticOverflow)?;
        let validator_fee = fee_taken - protocol_fee;

        unsafe {
            *fees_vault.borrow_mut_lamports_unchecked() += protocol_fee;
            *validator_fees_vault.borrow_mut_lamports_unchecked() += validator_fee;
        }
    }

    unsafe {
        *rent_reimbursement.borrow_mut_lamports_unchecked() += destination_amount;
        *target_account.borrow_mut_lamports_unchecked() = 0;
        target_account.assign(&pinocchio_system::ID);
    }
    target_account.resize(0).map_err(Into::into)
}
