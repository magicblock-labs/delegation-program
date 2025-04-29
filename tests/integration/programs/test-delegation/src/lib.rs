use anchor_lang::prelude::*;
use ephemeral_rollups_sdk_v2::anchor::{delegate, ephemeral};
use ephemeral_rollups_sdk_v2::cpi::DelegateConfig;

declare_id!("3vAK9JQiDsKoQNwmcfeEng4Cnv22pYuj1ASfso7U4ukF");

pub const TEST_PDA_SEED: &[u8] = b"test-pda";
pub const TEST_PDA_SEED_OTHER: &[u8] = b"test-pda-other";

#[ephemeral]
#[program]
pub mod test_delegation {
    use ephemeral_rollups_sdk_v2::pda::ephemeral_balance_pda_from_payer;
    use super::*;

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
        Ok(())
    }

    /// Handler for post commit action
    pub fn delegation_program_finalize_hook(ctx: Context<DelegationProgramFinalizeHook>, _data: Vec<u8>) -> Result<()> {
        // TODO: fix hardcoded index
        msg!("yay");
        let expected = ephemeral_balance_pda_from_payer(ctx.accounts.delegated_account.key, 0);
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

        Ok(())
    }
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
pub struct DelegationProgramFinalizeHook<'info> {
    pub delegated_account: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [b"balance", &delegated_account.key().as_ref(), &[0]],
        seeds::program = delegation_program_utils::ID,
        bump
    )]
    pub escrow_account: Signer<'info>,
    #[account(mut)]
    pub destination_account: AccountInfo<'info>,
}

#[account]
pub struct Counter {
    pub count: u64,
}

mod delegation_program_utils {
    use solana_program::declare_id;
    declare_id!("DELeGGvXpWV2fqJUhqcF5ZSYMS4JTLjteaAMARRSaeSh");
}
