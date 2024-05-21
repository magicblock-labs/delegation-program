use anchor_lang::prelude::*;
use anchor_lang::solana_program::system_program;
use bolt_lang::solana_program;

declare_id!("3vAK9JQiDsKoQNwmcfeEng4Cnv22pYuj1ASfso7U4ukF");

declare_program!(delegation);

pub const TEST_PDA_SEED: &[u8] = b"test-pda";
pub const BUFFER: &[u8] = b"buffer";

#[program]
pub mod test_delegation {
    use super::*;

    pub fn delegate(ctx: Context<DelegateInput>) -> Result<()> {
        let pda_signer_seeds: &[&[&[u8]]] = &[&[TEST_PDA_SEED, &[ctx.bumps.pda]]];
        let buffer_signer_seeds: &[&[&[u8]]] =
            &[&[BUFFER, ctx.accounts.pda.key.as_ref(), &[ctx.bumps.buffer]]];

        let data_len = ctx.accounts.pda.data_len();

        // Create the Buffer PDA
        create_pda(
            &ctx.accounts.buffer.to_account_info(),
            &id(),
            data_len,
            buffer_signer_seeds,
            &ctx.accounts.system_program.to_account_info(),
            &ctx.accounts.payer.to_account_info(),
        )?;

        // Copy the date to the buffer PDA
        let mut buffer_data = ctx.accounts.buffer.try_borrow_mut_data()?;
        let new_data = ctx.accounts.pda.try_borrow_data()?.to_vec().clone();
        (*buffer_data).copy_from_slice(&new_data);
        drop(buffer_data);

        // Close the PDA account
        close_account(&ctx.accounts.pda, &ctx.accounts.payer.to_account_info())?;

        // Re-create the PDA setting the delegation program as owner
        create_pda(
            &ctx.accounts.pda.to_account_info(),
            &ctx.accounts.delegation_program.key,
            data_len,
            pda_signer_seeds,
            &ctx.accounts.system_program.to_account_info(),
            &ctx.accounts.payer.to_account_info(),
        )?;

        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.delegation_program.to_account_info(),
            delegation::cpi::accounts::Delegate {
                payer: ctx.accounts.payer.to_account_info(),
                pda: ctx.accounts.pda.to_account_info(),
                owner_program: ctx.accounts.owner_program.to_account_info(),
                buffer: ctx.accounts.buffer.to_account_info(),
                delegation_record: ctx.accounts.delegation_record.to_account_info(),
                system_program: ctx.accounts.system_program.to_account_info(),
            },
            pda_signer_seeds,
        );
        delegation::cpi::delegate(cpi_ctx)?;
        close_account(&ctx.accounts.buffer, &ctx.accounts.payer.to_account_info())?;
        Ok(())
    }

    // Init a new Account
    pub fn process_undelegation(ctx: Context<InitializeAfterUndelegation>) -> Result<()> {
        if !ctx.accounts.buffer.is_signer {
            return Err(ProgramError::MissingRequiredSignature.into());
        }
        let mut data = ctx.accounts.base_account.try_borrow_mut_data()?;
        let buffer_data = ctx.accounts.buffer.try_borrow_data()?;
        (*data).copy_from_slice(&buffer_data);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct DelegateInput<'info> {
    pub payer: Signer<'info>,
    /// CHECK:
    #[account(mut, seeds = [TEST_PDA_SEED], bump)]
    pub pda: AccountInfo<'info>,
    /// CHECK:`
    pub owner_program: AccountInfo<'info>,
    /// CHECK:
    #[account(mut, seeds = [BUFFER, pda.key().as_ref()], bump)]
    pub buffer: AccountInfo<'info>,
    /// CHECK:`
    #[account(mut)]
    pub delegation_record: AccountInfo<'info>,
    /// CHECK:`
    pub delegation_program: AccountInfo<'info>,
    /// CHECK:`
    pub system_program: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct InitializeAfterUndelegation<'info> {
    /// CHECK:`
    #[account(init, payer = user, space = buffer.data_len(), seeds = [TEST_PDA_SEED], bump)]
    pub base_account: AccountInfo<'info>,
    /// CHECK:`
    #[account()]
    pub buffer: Signer<'info>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn close_account<'info>(
    info: &AccountInfo<'info>,
    sol_destination: &AccountInfo<'info>,
) -> Result<()> {
    // Transfer tokens from the account to the sol_destination.
    let dest_starting_lamports = sol_destination.lamports();
    **sol_destination.lamports.borrow_mut() =
        dest_starting_lamports.checked_add(info.lamports()).unwrap();
    **info.lamports.borrow_mut() = 0;
    info.assign(&system_program::ID);
    info.realloc(0, false).map_err(Into::into)
}

pub fn create_pda<'a, 'info>(
    target_account: &'a AccountInfo<'info>,
    owner: &Pubkey,
    space: usize,
    pda_seeds: &[&[&[u8]]],
    system_program: &'a AccountInfo<'info>,
    payer: &'a AccountInfo<'info>,
) -> Result<()> {
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
            pda_seeds,
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
            pda_seeds,
        )?;

        // 3) assign our program as the owner
        solana_program::program::invoke_signed(
            &solana_program::system_instruction::assign(target_account.key, owner),
            &[
                target_account.as_ref().clone(),
                system_program.as_ref().clone(),
            ],
            pda_seeds,
        )?;
    }

    Ok(())
}
