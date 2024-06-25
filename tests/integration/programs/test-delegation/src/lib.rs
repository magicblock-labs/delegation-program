use bolt_lang::*;
use delegation_program_sdk::{delegate, delegate_account};

declare_id!("3vAK9JQiDsKoQNwmcfeEng4Cnv22pYuj1ASfso7U4ukF");

pub const TEST_PDA_SEED: &[u8] = b"test-pda";

#[delegate]
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

    pub fn allow_undelegation(ctx: Context<AllowUndelegation>) -> Result<()> {
        let counter =
            Counter::try_deserialize_unchecked(&mut (&**ctx.accounts.counter.try_borrow_data()?))?;
        msg!("Counter: {:?}", counter.count);
        if counter.count > 0 {
            msg!("Counter is greater than 0, undelegation is allowed");
            delegation_program_sdk::allow_undelegation(
                &ctx.accounts.counter,
                &ctx.accounts.delegation_record,
                &ctx.accounts.delegation_metadata,
                &ctx.accounts.counter,
                &ctx.accounts.delegation_program,
                &id()
            )?;
        }
        Ok(())
    }

    /// Delegate the account to the delegation program
    pub fn delegate(ctx: Context<DelegateInput>) -> Result<()> {
        let pda_seeds: &[&[u8]] = &[TEST_PDA_SEED];

        let [payer, pda, owner_program, buffer, delegation_record, delegation_metadata, delegation_program, system_program] = [
            &ctx.accounts.payer,
            &ctx.accounts.pda,
            &ctx.accounts.owner_program,
            &ctx.accounts.buffer,
            &ctx.accounts.delegation_record,
            &ctx.accounts.delegation_metadata,
            &ctx.accounts.delegation_program,
            &ctx.accounts.system_program,
        ];

        delegate_account(
            payer,
            pda,
            owner_program,
            buffer,
            delegation_record,
            delegation_metadata,
            delegation_program,
            system_program,
            pda_seeds,
            0,
            30000,
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
    pub delegation_metadata: AccountInfo<'info>,
    /// CHECK:`
    pub delegation_program: AccountInfo<'info>,
    /// CHECK:`
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

#[derive(Accounts)]
pub struct AllowUndelegation<'info> {
    #[account(seeds = [TEST_PDA_SEED], bump)]
    /// CHECK: The counter pda
    pub counter: AccountInfo<'info>,
    #[account()]
    /// CHECK: delegation record
    pub delegation_record: AccountInfo<'info>,
    #[account(mut)]
    /// CHECK: delegation metadata
    pub delegation_metadata: AccountInfo<'info>,
    #[account()]
    /// CHECK: singer buffer to enforce CPI
    pub buffer: AccountInfo<'info>,
    #[account()]
    /// CHECK:`
    pub delegation_program: AccountInfo<'info>,
}

#[account]
pub struct Counter {
    pub count: u64,
}
