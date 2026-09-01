use pinocchio::{
    cpi::Signer,
    sysvars::{rent::Rent, Sysvar},
    AccountView, Address, ProgramResult,
};
use pinocchio_system::instructions as system;

use crate::consts::PROTOCOL_FEES_PERCENTAGE;

/// Creates a new pda
#[inline(always)]
pub(crate) fn create_pda(
    target_account: &AccountView,
    owner: &Address,
    space: usize,
    pda_signers: &[Signer],
    payer: &AccountView,
) -> ProgramResult {
    create_pda_with_rent_exempt_lamports(
        target_account,
        owner,
        space,
        Rent::get()?.try_minimum_balance(space)?,
        pda_signers,
        payer,
    )
}

/// Creates a new PDA with an explicit rent-exempt lamport target.
#[inline(always)]
pub(crate) fn create_pda_with_rent_exempt_lamports(
    target_account: &AccountView,
    owner: &Address,
    space: usize,
    rent_exempt_lamports: u64,
    pda_signers: &[Signer],
    payer: &AccountView,
) -> ProgramResult {
    // Create the account manually or using the create instruction
    if target_account.lamports().eq(&0) {
        // If balance is zero, create account
        system::CreateAccount {
            from: payer,
            to: target_account,
            lamports: rent_exempt_lamports,
            space: space as u64,
            owner,
        }
        .invoke_signed(pda_signers)
    } else {
        // Otherwise, if balance is nonzero:

        // 1) transfer sufficient lamports for rent exemption
        let rent_exempt_balance =
            rent_exempt_lamports.saturating_sub(target_account.lamports());
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

/// Tops up a PDA to the rent-exempt balance for `space`.
#[inline(always)]
pub(crate) fn top_up_pda_rent(
    payer: &AccountView,
    target_account: &AccountView,
    space: usize,
) -> ProgramResult {
    let rent = Rent::get()?;
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

    Ok(())
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
