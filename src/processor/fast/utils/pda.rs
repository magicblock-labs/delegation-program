use pinocchio::{
    account::MAX_PERMITTED_DATA_INCREASE,
    cpi::Signer,
    error::ProgramError,
    instruction::seeds,
    sysvars::{rent::Rent, Sysvar},
    AccountView, Address, ProgramResult,
};
use pinocchio_system::instructions as system;

use crate::{
    consts::PROTOCOL_FEES_PERCENTAGE,
    error::DlpError,
    requires::{
        require_initialized_pda, require_uninitialized_pda,
        RequireUninitializedAccountCtx,
    },
};

/// Creates a new pda
#[inline(always)]
pub(crate) fn create_pda(
    target_account: &AccountView,
    owner: &Address,
    space: usize,
    pda_signers: &[Signer],
    payer: &AccountView,
) -> ProgramResult {
    // Create the account manually or using the create instruction

    let rent = Rent::get()?;
    if target_account.lamports().eq(&0) {
        // If balance is zero, create account
        system::CreateAccount {
            from: payer,
            to: target_account,
            lamports: rent.try_minimum_balance(space)?,
            space: space as u64,
            owner,
        }
        .invoke_signed(pda_signers)
    } else {
        // Otherwise, if balance is nonzero:

        // 1) transfer sufficient lamports for rent exemption
        let rent_exempt_balance = rent
            .try_minimum_balance(space)?
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

/// Prepares `buffer_account` to hold exactly `state_size` bytes, seeded by
/// `seeds_arr` under this program. Returns the PDA's bump seed.
#[allow(clippy::too_many_arguments)]
pub(crate) fn create_or_verify_preallocated_pda(
    target_account: &AccountView,
    seeds_arr: &[&[u8]; 2],
    state_size: usize,
    payer: &AccountView,
    ctx: impl RequireUninitializedAccountCtx,
    label: &str,
) -> Result<u8, ProgramError> {
    if state_size > MAX_PERMITTED_DATA_INCREASE {
        // Account must be initialized by PreallocateBuffer instructions dues to `MAX_PERMITTED_DATA_INCREASE`.
        let bump = require_initialized_pda(
            target_account,
            seeds_arr,
            &crate::fast::ID,
            true,
            label,
        )?;

        if target_account.data_len() != state_size {
            Err(DlpError::BufferNotPreallocatedToExactSize.into())
        } else {
            Ok(bump)
        }
    } else {
        // As the account is small enough we initialize here
        let bump = require_uninitialized_pda(
            target_account,
            seeds_arr,
            &crate::fast::ID,
            true,
            ctx,
        )?;
        create_pda(
            target_account,
            &crate::fast::ID,
            state_size,
            &[Signer::from(&seeds!(seeds_arr[0], seeds_arr[1], &[bump]))],
            payer,
        )?;
        Ok(bump)
    }
}

/// Close PDA
#[inline(always)]
pub(crate) fn close_pda(
    target_account: &AccountView,
    destination: &AccountView,
) -> ProgramResult {
    // Transfer tokens from the account to the destination.

    destination
        .set_lamports(destination.lamports() + target_account.lamports());
    target_account.set_lamports(0);

    unsafe {
        target_account.assign(&pinocchio_system::ID);
    }

    target_account.resize(0)
}

/// Close PDA with fees, distributing the fees to the specified addresses in sequence
/// The total fees are calculated as `fee_percentage` of the total lamports in the PDA
/// Each fee address receives fee_percentage % of the previous fee address's amount
pub(crate) fn close_pda_with_fees(
    target_account: &AccountView,
    rent_reimbursement: &AccountView,
    fees_vault: &AccountView,
    validator_fees_vault: &AccountView,
    fee_remaining: &mut u64,
) -> ProgramResult {
    let mut destination_amount = target_account.lamports();

    if *fee_remaining > 0 && destination_amount > 0 {
        let fee_taken = (*fee_remaining).min(destination_amount);
        destination_amount -= fee_taken;
        *fee_remaining -= fee_taken;

        let protocol_fee = fee_taken * PROTOCOL_FEES_PERCENTAGE as u64 / 100;
        let validator_fee = fee_taken - protocol_fee;

        fees_vault.set_lamports(fees_vault.lamports() + protocol_fee);
        validator_fees_vault
            .set_lamports(validator_fees_vault.lamports() + validator_fee);
    }

    rent_reimbursement
        .set_lamports(rent_reimbursement.lamports() + destination_amount);
    target_account.set_lamports(0);

    unsafe {
        target_account.assign(&pinocchio_system::ID);
    }

    target_account.resize(0)
}

/// Resizes `pda` to `new_size`, topping it up with rent from `payer` if
/// needed. Returns the amount of lamports actually transferred (0 if the
/// account was already rent-exempt for `new_size`).
pub(crate) fn resize_pda(
    payer: &AccountView,
    pda: &AccountView,
    _system_program: &AccountView,
    new_size: usize,
) -> Result<u64, ProgramError> {
    let rent = Rent::get()?;
    let rent_exempt_balance = rent
        .try_minimum_balance(new_size)?
        .saturating_sub(pda.lamports());
    if rent_exempt_balance > 0 {
        system::Transfer {
            from: payer,
            to: pda,
            lamports: rent_exempt_balance,
        }
        .invoke()?;
    }
    pda.resize(new_size)?;
    Ok(rent_exempt_balance)
}
