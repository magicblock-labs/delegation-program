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
        let pda_seeds: &[&[u8]] = &[TEST_PDA_SEED];

        let buffer_seeds: &[&[u8]] = &[BUFFER, ctx.accounts.pda.key.as_ref()];

        let (_, delegate_account_bump) = Pubkey::find_program_address(pda_seeds, &id());

        let (_, buffer_pda_bump) = Pubkey::find_program_address(buffer_seeds, &id());

        // Pda signer seeds
        let delegate_account_bump_slice: &[u8] = &[delegate_account_bump];
        let pda_signer_seeds: &[&[&[u8]]] =
            &[&*seeds_with_bump(pda_seeds, delegate_account_bump_slice)];

        // Buffer signer seeds
        let buffer_bump_slice: &[u8] = &[buffer_pda_bump];
        let buffer_signer_seeds: &[&[&[u8]]] =
            &[&*seeds_with_bump(buffer_seeds, buffer_bump_slice)];

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
            ctx.accounts.delegation_program.key,
            data_len,
            pda_signer_seeds,
            &ctx.accounts.system_program.to_account_info(),
            &ctx.accounts.payer.to_account_info(),
        )?;

        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.delegation_program.to_account_info(),
            delegation::cpi::accounts::Delegate {
                payer: ctx.accounts.payer.to_account_info(),
                delegate_account: ctx.accounts.pda.to_account_info(),
                owner_program: ctx.accounts.owner_program.to_account_info(),
                buffer: ctx.accounts.buffer.to_account_info(),
                delegation_record: ctx.accounts.delegation_record.to_account_info(),
                delegate_account_seeds: ctx.accounts.delegate_account_seeds.to_account_info(),
                system_program: ctx.accounts.system_program.to_account_info(),
            },
            pda_signer_seeds,
        );

        let seeds_vec: Vec<Vec<u8>> = pda_seeds.iter().map(|&slice| slice.to_vec()).collect();
        delegation::cpi::delegate(cpi_ctx, 0, 3000, seeds_vec)?;
        close_account(&ctx.accounts.buffer, &ctx.accounts.payer.to_account_info())?;
        Ok(())
    }

    // Init a new Account
    pub fn process_undelegation(
        ctx: Context<InitializeAfterUndelegation>,
        account_seeds: Vec<Vec<u8>>,
    ) -> Result<()> {
        if !ctx.accounts.buffer.is_signer {
            return Err(ProgramError::MissingRequiredSignature.into());
        }

        let account_seeds: Vec<&[u8]> = account_seeds.iter().map(|v| v.as_slice()).collect();

        let (_, account_bump) = Pubkey::find_program_address(account_seeds.as_ref(), &id());

        // Account signer seeds
        let account_bump_slice: &[u8] = &[account_bump];
        let account_signer_seeds: &[&[&[u8]]] = &[&*seeds_with_bump(
            account_seeds.as_ref(),
            account_bump_slice,
        )];

        // Re-create the original PDA
        create_pda(
            &ctx.accounts.base_account.to_account_info(),
            &id(),
            ctx.accounts.buffer.data_len(),
            account_signer_seeds,
            &ctx.accounts.system_program.to_account_info(),
            &ctx.accounts.payer.to_account_info(),
        )?;

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
    #[account(mut)]
    pub pda: AccountInfo<'info>,
    /// CHECK:`
    pub owner_program: AccountInfo<'info>,
    /// CHECK:
    #[account(mut)]
    pub buffer: AccountInfo<'info>,
    /// CHECK:`
    #[account(mut)]
    pub delegation_record: AccountInfo<'info>,
    /// CHECK:`
    #[account(mut)]
    pub delegate_account_seeds: AccountInfo<'info>,
    /// CHECK:`
    pub delegation_program: AccountInfo<'info>,
    /// CHECK:`
    pub system_program: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct InitializeAfterUndelegation<'info> {
    /// CHECK:`
    #[account(mut)]
    pub base_account: AccountInfo<'info>,
    /// CHECK:`
    #[account()]
    pub buffer: Signer<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

fn seeds_with_bump<'a>(seeds: &'a [&'a [u8]], bump: &'a [u8]) -> Vec<&'a [u8]> {
    let mut combined: Vec<&'a [u8]> = Vec::with_capacity(seeds.len() + 1);
    combined.extend_from_slice(seeds);
    combined.push(bump);
    combined
}

pub fn close_account<'info>(
    info: &AccountInfo<'info>,
    sol_destination: &AccountInfo<'info>,
) -> Result<()> {
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
