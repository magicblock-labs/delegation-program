use bolt_lang::solana_program;
use bolt_lang::solana_program::entrypoint::ProgramResult;
use bolt_lang::solana_program::instruction::Instruction;
use bolt_lang::solana_program::system_program;
use bolt_lang::*;
use dlp::instruction::DelegateAccountArgs;

declare_id!("3vAK9JQiDsKoQNwmcfeEng4Cnv22pYuj1ASfso7U4ukF");

pub const TEST_PDA_SEED: &[u8] = b"test-pda";
pub const BUFFER: &[u8] = b"buffer";

#[program]
pub mod test_delegation {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        let counter = &mut ctx.accounts.counter;
        counter.count = 0;
        Ok(())
    }

    pub fn increment(ctx: Context<Increment>) -> Result<()> {
        let counter = &mut ctx.accounts.counter;
        counter.count += 1;
        Ok(())
    }

    /// Delegate the account to the delegation program, TODO: refactor to use an external crate
    pub fn delegate(ctx: Context<DelegateInput>) -> Result<()> {
        let pda_seeds: &[&[u8]] = &[TEST_PDA_SEED];

        let [payer, pda, owner_program, buffer, delegation_record, delegate_account_seeds, delegation_program, system_program] = [
            &ctx.accounts.payer,
            &ctx.accounts.pda,
            &ctx.accounts.owner_program,
            &ctx.accounts.buffer,
            &ctx.accounts.delegation_record,
            &ctx.accounts.delegate_account_seeds,
            &ctx.accounts.delegation_program,
            &ctx.accounts.system_program,
        ];

        delegate_account(
            payer,
            pda,
            owner_program,
            buffer,
            delegation_record,
            delegate_account_seeds,
            delegation_program,
            system_program,
            pda_seeds,
        )?;

        Ok(())
    }

    /// Undelegate the account, TODO: refactor to use an external crate
    pub fn process_undelegation(
        ctx: Context<InitializeAfterUndelegation>,
        account_seeds: Vec<Vec<u8>>,
    ) -> Result<()> {
        let [delegated_account, buffer, payer, system_program] = [
            &ctx.accounts.base_account,
            &ctx.accounts.buffer,
            &ctx.accounts.payer,
            &ctx.accounts.system_program,
        ];

        undelegate_account(
            delegated_account,
            buffer,
            payer,
            system_program,
            account_seeds,
        )?;

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
    pub buffer: AccountInfo<'info>,
    /// CHECK:
    #[account(mut)]
    pub payer: AccountInfo<'info>,
    /// CHECK:
    pub system_program: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, payer = user, space = 8 + 8, seeds = [TEST_PDA_SEED], bump)]
    pub counter: Account<'info, Counter>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Increment<'info> {
    #[account(mut, seeds = [TEST_PDA_SEED], bump)]
    pub counter: Account<'info, Counter>,
}

#[account]
pub struct Counter {
    pub count: u64,
}

#[allow(clippy::too_many_arguments)]
pub fn delegate_account<'a, 'info>(
    payer: &'a AccountInfo<'info>,
    pda: &'a AccountInfo<'info>,
    owner_program: &'a AccountInfo<'info>,
    buffer: &'a AccountInfo<'info>,
    delegation_record: &'a AccountInfo<'info>,
    delegate_account_seeds: &'a AccountInfo<'info>,
    delegation_program: &'a AccountInfo<'info>,
    system_program: &'a AccountInfo<'info>,
    pda_seeds: &[&[u8]],
) -> ProgramResult {
    let buffer_seeds: &[&[u8]] = &[BUFFER, pda.key.as_ref()];

    let (_, delegate_account_bump) = Pubkey::find_program_address(pda_seeds, &id());

    let (_, buffer_pda_bump) = Pubkey::find_program_address(buffer_seeds, &id());

    // Pda signer seeds
    let delegate_account_bump_slice: &[u8] = &[delegate_account_bump];
    let pda_signer_seeds: &[&[&[u8]]] =
        &[&*seeds_with_bump(pda_seeds, delegate_account_bump_slice)];

    // Buffer signer seeds
    let buffer_bump_slice: &[u8] = &[buffer_pda_bump];
    let buffer_signer_seeds: &[&[&[u8]]] = &[&*seeds_with_bump(buffer_seeds, buffer_bump_slice)];

    let data_len = pda.data_len();

    // Create the Buffer PDA
    create_pda(
        buffer,
        &id(),
        data_len,
        buffer_signer_seeds,
        system_program,
        payer,
    )?;

    // Copy the date to the buffer PDA
    let mut buffer_data = buffer.try_borrow_mut_data()?;
    let new_data = pda.try_borrow_data()?.to_vec().clone();
    (*buffer_data).copy_from_slice(&new_data);
    drop(buffer_data);

    // Close the PDA account
    close_account(pda, payer)?;

    // Re-create the PDA setting the delegation program as owner
    create_pda(
        pda,
        delegation_program.key,
        data_len,
        pda_signer_seeds,
        system_program,
        payer,
    )?;

    let seeds_vec: Vec<Vec<u8>> = pda_seeds.iter().map(|&slice| slice.to_vec()).collect();

    let delegation_args = DelegateAccountArgs {
        valid_until: 0,
        commit_frequency_ms: 1000,
        seeds: seeds_vec,
    };

    cpi_delegate(
        payer,
        pda,
        owner_program,
        buffer,
        delegation_record,
        delegate_account_seeds,
        system_program,
        pda_signer_seeds,
        delegation_args,
    )?;

    close_account(buffer, payer)?;
    Ok(())
}

pub fn undelegate_account<'a, 'info>(
    delegated_account: &'a AccountInfo<'info>,
    buffer: &'a AccountInfo<'info>,
    payer: &'a AccountInfo<'info>,
    system_program: &'a AccountInfo<'info>,
    account_signer_seeds: Vec<Vec<u8>>,
) -> ProgramResult {
    if !buffer.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let account_seeds: Vec<&[u8]> = account_signer_seeds.iter().map(|v| v.as_slice()).collect();

    let (_, account_bump) = Pubkey::find_program_address(account_seeds.as_ref(), &id());

    // Account signer seeds
    let account_bump_slice: &[u8] = &[account_bump];
    let account_signer_seeds: &[&[&[u8]]] = &[&*seeds_with_bump(
        account_seeds.as_ref(),
        account_bump_slice,
    )];

    // Re-create the original PDA
    create_pda(
        &delegated_account.to_account_info(),
        &id(),
        buffer.data_len(),
        account_signer_seeds,
        &system_program.to_account_info(),
        &payer.to_account_info(),
    )?;

    let mut data = delegated_account.try_borrow_mut_data()?;
    let buffer_data = buffer.try_borrow_data()?;
    (*data).copy_from_slice(&buffer_data);
    Ok(())
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

#[allow(clippy::too_many_arguments)]
pub fn cpi_delegate<'a, 'info>(
    payer: &'a AccountInfo<'info>,
    delegate_account: &'a AccountInfo<'info>,
    owner_program: &'a AccountInfo<'info>,
    buffer: &'a AccountInfo<'info>,
    delegation_record: &'a AccountInfo<'info>,
    delegated_account_seeds: &'a AccountInfo<'info>,
    system_program: &'a AccountInfo<'info>,
    signers_seeds: &[&[&[u8]]],
    args: DelegateAccountArgs,
) -> ProgramResult {
    let mut data: Vec<u8> = vec![0u8; 8];
    let serialized_seeds = args.try_to_vec()?;
    data.extend_from_slice(&serialized_seeds);
    msg!("Data: {:?}", data);

    let delegation_instruction = Instruction {
        program_id: dlp::id(),
        accounts: vec![
            AccountMeta {
                pubkey: *payer.key,
                is_signer: true,
                is_writable: true,
            },
            AccountMeta {
                pubkey: *delegate_account.key,
                is_signer: true,
                is_writable: true,
            },
            AccountMeta {
                pubkey: *owner_program.key,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: *buffer.key,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: *delegation_record.key,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: *delegated_account_seeds.key,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: *system_program.key,
                is_signer: false,
                is_writable: false,
            },
        ],
        data,
    };

    solana_program::program::invoke_signed(
        &delegation_instruction,
        &[
            payer.clone(),
            delegate_account.clone(),
            owner_program.clone(),
            buffer.clone(),
            delegation_record.clone(),
            delegated_account_seeds.clone(),
            system_program.clone(),
        ],
        signers_seeds,
    )
}
