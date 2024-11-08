use solana_curve25519::edwards::{validate_edwards, PodEdwardsPoint};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey, rent::Rent,
    sysvar::Sysvar,
};

/// Creates a new pda
#[inline(always)]
pub(crate) fn create_pda<'a, 'info>(
    target_account: &'a AccountInfo<'info>,
    owner: &Pubkey,
    space: usize,
    pda_seeds: &[&[u8]],
    system_program: &'a AccountInfo<'info>,
    payer: &'a AccountInfo<'info>,
) -> ProgramResult {
    let rent = Rent::get()?;
    if target_account.lamports().eq(&0) {
        // If balance is zero, create account
        solana_program::program::invoke_signed(
            &solana_program::system_instruction::create_account(
                payer.key,
                target_account.key,
                rent.minimum_balance(space),
                space as u64,
                owner,
            ),
            &[
                payer.clone(),
                target_account.clone(),
                system_program.clone(),
            ],
            &[pda_seeds],
        )?;
    } else {
        // Otherwise, if balance is nonzero:
        // 1) transfer sufficient lamports for rent exemption
        let rent_exempt_balance = rent
            .minimum_balance(space)
            .saturating_sub(target_account.lamports());
        if rent_exempt_balance.gt(&0) {
            solana_program::program::invoke(
                &solana_program::system_instruction::transfer(
                    payer.key,
                    target_account.key,
                    rent_exempt_balance,
                ),
                &[
                    payer.as_ref().clone(),
                    target_account.as_ref().clone(),
                    system_program.as_ref().clone(),
                ],
            )?;
        }

        // 2) allocate space for the account
        solana_program::program::invoke_signed(
            &solana_program::system_instruction::allocate(target_account.key, space as u64),
            &[
                target_account.as_ref().clone(),
                system_program.as_ref().clone(),
            ],
            &[pda_seeds],
        )?;

        // 3) assign our program as the owner
        solana_program::program::invoke_signed(
            &solana_program::system_instruction::assign(target_account.key, owner),
            &[
                target_account.as_ref().clone(),
                system_program.as_ref().clone(),
            ],
            &[pda_seeds],
        )?;
    }

    Ok(())
}

/// Close PDA
#[inline(always)]
pub(crate) fn close_pda<'a, 'info>(
    target_account: &'a AccountInfo<'info>,
    destination: &'a AccountInfo<'info>,
) -> ProgramResult {
    // Transfer tokens from the account to the destination.
    let dest_starting_lamports = destination.lamports();
    **destination.lamports.borrow_mut() = dest_starting_lamports
        .checked_add(target_account.lamports())
        .unwrap();
    **target_account.lamports.borrow_mut() = 0;

    target_account.assign(&solana_program::system_program::ID);
    target_account.realloc(0, false).map_err(Into::into)
}

/// Close PDA with fees, distributing the fees to the specified addresses in sequence
#[inline(always)]
pub(crate) fn close_pda_with_fees<'a, 'info>(
    target_account: &'a AccountInfo<'info>,
    destination: &'a AccountInfo<'info>,
    fees_addresses: &[&AccountInfo<'info>],
    fee_percentage: u8,
) -> ProgramResult {
    let total_lamports = target_account.lamports();
    let mut remaining_amount = total_lamports;

    for fee_address in fees_addresses.iter() {
        let fee_amount = (remaining_amount as u128)
            .checked_mul(fee_percentage as u128)
            .unwrap()
            .checked_div(100)
            .unwrap() as u64;

        let fees_starting_lamports = fee_address.lamports();
        **fee_address.lamports.borrow_mut() =
            fees_starting_lamports.checked_add(fee_amount).unwrap();

        remaining_amount = remaining_amount.checked_sub(fee_amount).unwrap();

        if remaining_amount == 0 {
            break;
        }
    }

    let dest_starting_lamports = destination.lamports();
    **destination.lamports.borrow_mut() = dest_starting_lamports
        .checked_add(remaining_amount)
        .unwrap();

    **target_account.lamports.borrow_mut() = 0;
    target_account.assign(&solana_program::system_program::ID);
    target_account.realloc(0, false).map_err(Into::into)
}

/// Define a trait to add is_on_curve method to AccountInfo
pub trait ValidateEdwards {
    fn is_on_curve(&self) -> bool;
}

/// Implement the trait for AccountInfo
impl ValidateEdwards for AccountInfo<'_> {
    fn is_on_curve(&self) -> bool {
        validate_edwards(&PodEdwardsPoint(self.key.to_bytes()))
    }
}
