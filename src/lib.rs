use solana_program::{
    self, account_info::AccountInfo, declare_id, entrypoint::ProgramResult, msg,
    program_error::ProgramError, pubkey::Pubkey,
};

use instruction::*;
use processor::*;

pub mod consts;
pub mod error;
pub mod instruction;
pub mod pda;
mod processor;
pub mod state;
pub mod utils;

declare_id!("DELeGGvXpWV2fqJUhqcF5ZSYMS4JTLjteaAMARRSaeSh");

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(process_instruction);

#[cfg(all(not(feature = "no-entrypoint"), feature = "solana-security-txt"))]
solana_security_txt::security_txt! {
    name: "MagicBlock Delegation Program",
    project_url: "https://magicblock.gg",
    contacts: "email:dev@magicblock.gg,twitter:@magicblock",
    policy: "https://github.com/magicblock-labs/delegation-program/blob/master/LICENSE.md",
    preferred_languages: "en",
    source_code: "https://github.com/magicblock-labs/delegation-program"
}

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    if program_id.ne(&id()) {
        return Err(ProgramError::IncorrectProgramId);
    }

    if data.len() < 8 {
        return Err(ProgramError::InvalidInstructionData);
    }

    let (tag, data) = data.split_at(8);
    let tag_array: [u8; 8] = tag
        .try_into()
        .map_err(|_| ProgramError::InvalidInstructionData)?;

    let ix = DlpInstruction::try_from(tag_array).or(Err(ProgramError::InvalidInstructionData))?;
    msg!("Processing instruction: {:?}", ix);
    match ix {
        DlpInstruction::Delegate => process_delegate(program_id, accounts, data)?,
        DlpInstruction::CommitState => process_commit_state(program_id, accounts, data)?,
        DlpInstruction::Finalize => process_finalize(program_id, accounts, data)?,
        DlpInstruction::Undelegate => process_undelegate(program_id, accounts, data)?,
        DlpInstruction::AllowUndelegate => process_allow_undelegate(program_id, accounts, data)?,
        DlpInstruction::InitValidatorFeesVault => {
            process_init_validator_fees_vault(program_id, accounts, data)?
        }
        DlpInstruction::InitFeesVault => process_init_fees_vault(program_id, accounts, data)?,
        DlpInstruction::TopUpEphemeralBalance => {
            process_top_up_ephemeral(program_id, accounts, data)?
        }
        DlpInstruction::WithdrawEphemeralBalance => process_withdraw(program_id, accounts, data)?,
    }
    Ok(())
}
