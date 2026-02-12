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
        let counter = &mut ctx.accounts.counter;
        counter.value = 0;
        Ok(())
    }

    /// Increment counter by 1
    pub fn increment(ctx: Context<Increment>) -> Result<()> {
        let counter = &mut ctx.accounts.counter;

        counter.value =
            counter.value.checked_add(1).ok_or(ErrorCode::Overflow)?;

        Ok(())
    }

    /// (Optional) Reset to zero (useful in tests)
    pub fn reset(ctx: Context<Increment>) -> Result<()> {
        let counter = &mut ctx.accounts.counter;
        counter.value = 0;
        Ok(())
    }
}

/* ================================
   Accounts
================================ */

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

/* ================================
   State
================================ */

#[account]
pub struct Counter {
    pub value: u64,
}

impl Counter {
    pub const SIZE: usize = 8; // u64
}

/* ================================
   Errors
================================ */

#[error_code]
pub enum ErrorCode {
    #[msg("Counter overflow")]
    Overflow,
}
