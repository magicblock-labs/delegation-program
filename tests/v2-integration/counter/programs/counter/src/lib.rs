use anchor_lang::prelude::*;
use ephemeral_rollups_sdk::{
    anchor::{delegate, ephemeral},
    cpi::DelegateConfig,
};

declare_id!("8Aw8uKuJL2Yhr7nNCYjKAtKAajyoRicCbipR1kT3qEmW");

#[ephemeral]
#[program]
pub mod counter {
    use super::*;

    /// Create the PDA and initialize counter = 0
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("user: {}", ctx.accounts.user.key);
        let counter = &mut ctx.accounts.counter;
        counter.value = 0;
        Ok(())
    }

    pub fn increment(ctx: Context<Increment>) -> Result<()> {
        let counter = &mut ctx.accounts.counter;

        counter.value =
            counter.value.checked_add(1).ok_or(ErrorCode::Overflow)?;

        Ok(())
    }

    pub fn delegate_counter(ctx: Context<DelegateCounter>) -> Result<()> {
        ctx.accounts
            .delegate_pda(
                &ctx.accounts.payer,
                &[],
                DelegateConfig {
                    commit_frequency_ms: 1000,
                    validator: None, //"mAGicPQYBMvcYveUZA5F5UNNwyHvfYh5xkLS2Fr1mev"),
                },
            )
            .unwrap();
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = user,
        space = 8 + Counter::SIZE,
        seeds = [b"counter", user.key().as_ref()],
        bump
    )]
    pub counter: Account<'info, Counter>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[delegate]
#[derive(Accounts)]
pub struct DelegateCounter<'info> {
    pub payer: Signer<'info>,
    /// CHECK: The pda to delegate
    #[account(mut, del, seeds = [b"counter", payer.key().as_ref()], bump)]
    pub pda: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct Increment<'info> {
    #[account(
        mut,
        seeds = [b"counter", user.key().as_ref()],
        bump
    )]
    pub counter: Account<'info, Counter>,

    pub user: Signer<'info>,
}

#[account]
pub struct Counter {
    pub value: u64,
}

impl Counter {
    pub const SIZE: usize = 8; // u64
}

#[error_code]
pub enum ErrorCode {
    #[msg("Counter overflow")]
    Overflow,
}
