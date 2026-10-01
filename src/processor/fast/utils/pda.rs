use pinocchio::{
    cpi::Signer,
    error::ProgramError,
    sysvars::{rent::Rent, Sysvar},
    AccountView, Address, ProgramResult,
};
use pinocchio_system::instructions as system;

use crate::{consts::PROTOCOL_FEES_PERCENTAGE, error::DlpError};

// Legacy rent math follows SIMD-0194, which defines the 6,960
// lamports-per-byte value used by the simplified rent-exemption formula.
//
// ref:
// https://github.com/solana-foundation/solana-improvement-documents/blob/main/proposals/0194-deprecate-rent-exemption-threshold.md
const LEGACY_RENT_EXEMPT_LAMPORTS_PER_BYTE: u64 = 6960;
const LEGACY_RENT_ACCOUNT_STORAGE_OVERHEAD: u64 = 128;

// AccountSpace represents the account allocation size.
// Its variant lets the caller choose how rent funding should be computed for that size.
pub(crate) enum AccountSpace {
    CurrentRent(usize),
    LegacyRent(usize),
}

impl AccountSpace {
    fn space(&self) -> usize {
        match self {
            Self::CurrentRent(space) | Self::LegacyRent(space) => *space,
        }
    }

    fn minimum_balance(&self) -> Result<u64, ProgramError> {
        let current_rent = Rent::get()?.try_minimum_balance(self.space())?;
        match self {
            Self::CurrentRent(_) => Ok(current_rent),
            // Legacy rent is a fee-budget floor, not a substitute for current
            // rent exemption. Keep live rent as the lower bound if it ever
            // exceeds the legacy formula.
            Self::LegacyRent(_) => {
                Ok(current_rent.max(legacy_rent(self.space())?))
            }
        }
    }
}

// ref:
// https://github.com/solana-foundation/solana-improvement-documents/blob/main/proposals/0194-deprecate-rent-exemption-threshold.md
fn legacy_rent(space: usize) -> Result<u64, ProgramError> {
    let space = u64::try_from(space).map_err(|_| DlpError::Overflow)?;
    space
        .checked_add(LEGACY_RENT_ACCOUNT_STORAGE_OVERHEAD)
        .and_then(|bytes| {
            bytes.checked_mul(LEGACY_RENT_EXEMPT_LAMPORTS_PER_BYTE)
        })
        .ok_or(DlpError::Overflow.into())
}

/// Creates a new pda
#[inline(always)]
pub(crate) fn create_pda(
    target_account: &AccountView,
    owner: &Address,
    account_space: AccountSpace,
    pda_signers: &[Signer],
    payer: &AccountView,
) -> ProgramResult {
    // Create the account manually or using the create instruction

    if target_account.lamports().eq(&0) {
        // If balance is zero, create account
        system::CreateAccount {
            from: payer,
            to: target_account,
            lamports: account_space.minimum_balance()?,
            space: account_space.space() as u64,
            owner,
        }
        .invoke_signed(pda_signers)
    } else {
        // Otherwise, if balance is nonzero:

        // 1) transfer sufficient lamports for rent exemption
        let rent_exempt_balance = account_space
            .minimum_balance()?
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
            space: account_space.space() as u64,
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

#[cfg(test)]
mod tests {
    use super::legacy_rent;
    use crate::{
        state::{DelegationMetadata, DelegationRecord, UndelegationRequester},
        Pubkey,
    };

    #[test]
    fn legacy_rent_uses_pre_reduction_rent_exempt_formula() {
        let metadata = DelegationMetadata {
            last_commit_id: 0,
            undelegation_requester: UndelegationRequester::None,
            seeds: vec![b"test-pda".to_vec()],
            rent_payer: Pubkey::default(),
        };

        let delegation_rent =
            legacy_rent(DelegationRecord::size_with_discriminator()).unwrap()
                + legacy_rent(metadata.serialized_size()).unwrap();

        assert_eq!(delegation_rent, 2_902_320);
    }
}
