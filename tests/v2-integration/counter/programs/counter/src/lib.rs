use anchor_lang::prelude::*;

declare_id!("8Aw8uKuJL2Yhr7nNCYjKAtKAajyoRicCbipR1kT3qEmW");

#[program]
pub mod counter {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("Greetings from: {:?}", ctx.program_id);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
