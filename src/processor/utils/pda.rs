use solana_program::program::invoke;
use solana_program::program_error::ProgramError;
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey, rent::Rent,
    system_instruction, sysvar::Sysvar,
};

/// Creates a new pda
#[inline(always)]
pub(crate) fn create_pda<'a, 'info>(
    target_account: &'a AccountInfo<'info>,
    owner: &Pubkey,
    space: usize,
    pda_seeds: &[&[u8]],
    pda_bump: u8,
    system_program: &'a AccountInfo<'info>,
    payer: &'a AccountInfo<'info>,
) -> ProgramResult {
    // Generate the PDA's signer seeds
    let pda_bump_slice = &[pda_bump];
    let pda_signer_seeds = [pda_seeds, &[pda_bump_slice]].concat();
    // Create the account manually or using the create instruction
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
            &[&pda_signer_seeds],
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
            &[&pda_signer_seeds],
        )?;
        // 3) assign our program as the owner
        solana_program::program::invoke_signed(
            &solana_program::system_instruction::assign(target_account.key, owner),
            &[
                target_account.as_ref().clone(),
                system_program.as_ref().clone(),
            ],
            &[&pda_signer_seeds],
        )?;
    }

    Ok(())
}

/// Resize PDA
pub(crate) fn resize_pda<'a, 'info>(
    payer: &'a AccountInfo<'info>,
    pda: &'a AccountInfo<'info>,
    system_program: &'a AccountInfo<'info>,
    new_size: usize,
) -> Result<(), ProgramError> {
    let new_minimum_balance = Rent::default().minimum_balance(new_size);
    let lamports_diff = new_minimum_balance.saturating_sub(pda.lamports());
    invoke(
        &system_instruction::transfer(payer.key, pda.key, lamports_diff),
        &[payer.clone(), pda.clone(), system_program.clone()],
    )?;

    pda.realloc(new_size, false)?;
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
/// The total fees are calculated as `fee_percentage` of the total lamports in the PDA
/// Each fee address receives fee_percentage % of the previous fee address's amount
pub(crate) fn close_pda_with_fees<'a, 'info>(
    target_account: &'a AccountInfo<'info>,
    destination: &'a AccountInfo<'info>,
    fees_addresses: &[&AccountInfo<'info>],
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
        **fee_address.lamports.borrow_mut() = fee_address
            .lamports()
            .checked_add(fees[i])
            .ok_or(ProgramError::InsufficientFunds)?;
    }

    **destination.lamports.borrow_mut() = destination
        .lamports()
        .checked_add(init_lamports - total_fee_amount)
        .ok_or(ProgramError::InsufficientFunds)?;

    **target_account.lamports.borrow_mut() = 0;
    target_account.assign(&solana_program::system_program::ID);
    target_account.realloc(0, false).map_err(Into::into)
}
