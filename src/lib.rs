pub mod consts;
pub mod error;
pub mod instruction;
mod loaders;
mod processor;
pub mod state;
pub mod utils;

use instruction::*;
use processor::*;
use solana_program::{
    self, account_info::AccountInfo, declare_id, entrypoint::ProgramResult, msg,
    program_error::ProgramError, pubkey::Pubkey,
};

declare_id!("DELeGGvXpWV2fqJUhqcF5ZSYMS4JTLjteaAMARRSaeSh");

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(process_instruction);

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    if program_id.ne(&crate::id()) {
        return Err(ProgramError::IncorrectProgramId);
    }

    let (tag, data) = data
        .split_first()
        .ok_or(ProgramError::InvalidInstructionData)?;

    msg!("Processing instruction: {:?}", tag);
    msg!("data: {:?}", data);

    match DlpInstruction::try_from(*tag).or(Err(ProgramError::InvalidInstructionData))? {
        DlpInstruction::Delegate => process_delegate(program_id, accounts, data)?,
        DlpInstruction::CommitState => process_commit_state(program_id, accounts, data)?,
        DlpInstruction::Undelegate => todo!(),
    }

    Ok(())
}
