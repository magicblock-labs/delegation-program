use anchor_lang::prelude::*;
use ephemeral_rollups_sdk::anchor::{delegate, ephemeral};
use ephemeral_rollups_sdk::cpi::DelegateConfig;
use ephemeral_rollups_sdk::pda::ephemeral_balance_pda_from_payer;

declare_id!("3vAK9JQiDsKoQNwmcfeEng4Cnv22pYuj1ASfso7U4ukF");

pub const TEST_PDA_SEED: &[u8] = b"test-pda";
pub const TEST_PDA_SEED_OTHER: &[u8] = b"test-pda-other";

#[ephemeral]
#[program]
pub mod test_delegation {
    use super::*;
    use anchor_lang::system_program::{transfer, Transfer};

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        let counter = &mut ctx.accounts.counter;
        counter.count = 0;
        Ok(())
    }

    pub fn initialize_other(ctx: Context<InitializeOther>) -> Result<()> {
        let counter = &mut ctx.accounts.counter;
        counter.count = 0;
        Ok(())
    }

    pub fn increment(ctx: Context<Increment>) -> Result<()> {
        let counter = &mut ctx.accounts.counter;
        counter.count += 1;
        Ok(())
    }

    /// Delegate the account to the delegation program
    pub fn delegate(ctx: Context<DelegateInput>) -> Result<()> {
        ctx.accounts.delegate_pda(
            &ctx.accounts.payer,
            &[TEST_PDA_SEED],
            DelegateConfig::default(),
        )?;
        Ok(())
    }

    /// Delegate two accounts to the delegation program
    pub fn delegate_two(ctx: Context<DelegateInputTwo>) -> Result<()> {
        ctx.accounts.delegate_pda(
            &ctx.accounts.payer,
            &[TEST_PDA_SEED],
            DelegateConfig::default(),
        )?;
        ctx.accounts.delegate_pda_other(
            &ctx.accounts.payer,
            &[TEST_PDA_SEED_OTHER],
            DelegateConfig::default(),
        )?;
        msg!(
            "Delegated {:?}, owner {:?}",
            ctx.accounts.pda.key(),
            ctx.accounts.pda.owner
        );
        msg!(
            "Delegated {:?}, owner {:?}",
            ctx.accounts.pda_other.key(),
            ctx.accounts.pda_other.owner
        );
        Ok(())
    }

    /// Delegation program call handler
    pub fn delegation_program_call_handler(
        ctx: Context<DelegationProgramCallHandler>,
        hook_args: delegation_program_utils::CallHandlerArgs,
    ) -> Result<()> {
        let expected = ephemeral_balance_pda_from_payer(
            ctx.accounts.escrow_authority.key,
            hook_args.escrow_index,
        );
        if &expected != ctx.accounts.escrow_account.key {
            Err(ProgramError::InvalidAccountData)
        } else {
            Ok(())
        }?;

        if !ctx.accounts.escrow_account.is_signer {
            Err(ProgramError::MissingRequiredSignature)
        } else {
            Ok(())
        }?;

        match hook_args.context {
            delegation_program_utils::Context::Commit => {
                msg!("commit context");
                let amount = u64::try_from_slice(&hook_args.data)?;
                let transfer_ctx = CpiContext::new(
                    ctx.accounts.system_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.escrow_account.to_account_info(),
                        to: ctx.accounts.destination_account.to_account_info(),
                    },
                );
                transfer(transfer_ctx, amount)?;
            }
            delegation_program_utils::Context::Undelegate => {
                msg!("undelegate context");
                let amount = u64::try_from_slice(&hook_args.data)?;
                let transfer_ctx = CpiContext::new(
                    ctx.accounts.system_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.escrow_account.to_account_info(),
                        to: ctx.accounts.destination_account.to_account_info(),
                    },
                );
                transfer(transfer_ctx, amount)?;

                let counter_data = &mut ctx.accounts.counter.try_borrow_mut_data()?;
                let mut counter = Counter::try_from_slice(&counter_data)?;
                counter.count += 1;

                counter_data.copy_from_slice(&counter.try_to_vec()?);
            }
            delegation_program_utils::Context::Standalone => msg!("standalone context"),
        }

        Ok(())
    }
}

pub fn transfer_from_undelegated(
    undelegated_pda: &UncheckedAccount,
    destination_pda: &AccountInfo,
    amount: u64,
) -> Result<()> {
    if undelegated_pda.owner != &ID {
        return Err(ProgramError::IllegalOwner.into());
    }

    **undelegated_pda.try_borrow_mut_lamports()? = undelegated_pda
        .lamports()
        .checked_sub(amount)
        .ok_or(ProgramError::InsufficientFunds)?;

    **destination_pda.try_borrow_mut_lamports()? = destination_pda
        .lamports()
        .checked_add(amount)
        .ok_or(ProgramError::ArithmeticOverflow)?;

    Ok(())
}

#[delegate]
#[derive(Accounts)]
pub struct DelegateInput<'info> {
    pub payer: Signer<'info>,
    /// CHECK: The pda to delegate
    #[account(mut, del, seeds = [TEST_PDA_SEED], bump)]
    pub pda: AccountInfo<'info>,
}

#[delegate]
#[derive(Accounts)]
pub struct DelegateInputTwo<'info> {
    pub payer: Signer<'info>,
    /// CHECK: The pda to delegate
    #[account(mut, del, seeds = [TEST_PDA_SEED], bump)]
    pub pda: AccountInfo<'info>,
    /// CHECK: The other pda to delegate
    #[account(mut, del, seeds = [TEST_PDA_SEED_OTHER], bump)]
    pub pda_other: AccountInfo<'info>,
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
pub struct InitializeOther<'info> {
    #[account(init, payer = user, space = 8 + 8, seeds = [TEST_PDA_SEED_OTHER], bump)]
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

#[derive(Accounts)]
#[instruction(hook_args: delegation_program_utils::CallHandlerArgs)]
pub struct DelegationProgramCallHandler<'info> {
    /// CHECK: The authority that owns the escrow account
    pub escrow_authority: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [b"balance", &escrow_authority.key().as_ref(), &[hook_args.escrow_index]],
        seeds::program = delegation_program_utils::ID,
        bump
    )]
    pub escrow_account: Signer<'info>,
    /// CHECK: The destination account to transfer lamports to
    #[account(mut)]
    pub destination_account: AccountInfo<'info>,
    /// CHECK: fails in finalize stage due to ownership by dlp
    pub counter: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

#[account]
pub struct Counter {
    pub count: u64,
}

mod delegation_program_utils {
    use anchor_lang::prelude::*;

    declare_id!("DELeGGvXpWV2fqJUhqcF5ZSYMS4JTLjteaAMARRSaeSh");

    #[derive(AnchorSerialize, AnchorDeserialize)]
    pub enum Context {
        Commit,
        Undelegate,
        Standalone,
    }

    #[derive(AnchorSerialize, AnchorDeserialize)]
    pub struct CallHandlerArgs {
        pub escrow_index: u8,
        pub data: Vec<u8>,
        pub context: Context,
    }
}
