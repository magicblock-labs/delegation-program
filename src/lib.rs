#![allow(unexpected_cfgs)] // silence clippy for target_os solana and other solana program custom features

use solana_program::{
    account_info::AccountInfo, declare_id, entrypoint::ProgramResult, msg,
    program_error::ProgramError, pubkey::Pubkey,
};

pub mod args;
pub mod consts;
mod discriminator;
pub mod error;
pub mod instruction_builder;
pub mod pda;
mod processor;
pub mod state;

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

    let ix = discriminator::DlpDiscriminator::try_from(tag_array)
        .or(Err(ProgramError::InvalidInstructionData))?;
    msg!("Processing instruction: {:?}", ix);
    match ix {
        discriminator::DlpDiscriminator::Delegate => {
            processor::process_delegate(program_id, accounts, data)?
        }
        discriminator::DlpDiscriminator::CommitState => {
            processor::process_commit_state(program_id, accounts, data)?
        }
        discriminator::DlpDiscriminator::Finalize => {
            processor::process_finalize(program_id, accounts, data)?
        }
        discriminator::DlpDiscriminator::Undelegate => {
            processor::process_undelegate(program_id, accounts, data)?
        }
        discriminator::DlpDiscriminator::InitValidatorFeesVault => {
            processor::process_init_validator_fees_vault(program_id, accounts, data)?
        }
        discriminator::DlpDiscriminator::InitFeesVault => {
            processor::process_init_protocol_fees_vault(program_id, accounts, data)?
        }
        discriminator::DlpDiscriminator::ValidatorClaimFees => {
            processor::process_validator_claim_fees(program_id, accounts, data)?
        }
        discriminator::DlpDiscriminator::WhitelistValidatorForProgram => {
            processor::process_whitelist_validator_for_program(program_id, accounts, data)?
        }
        discriminator::DlpDiscriminator::TopUpEphemeralBalance => {
            processor::process_top_up_ephemeral_balance(program_id, accounts, data)?
        }
        discriminator::DlpDiscriminator::DelegateEphemeralBalance => {
            processor::process_delegate_ephemeral_balance(program_id, accounts, data)?
        }
        discriminator::DlpDiscriminator::CloseEphemeralBalance => {
            processor::process_close_ephemeral_balance(program_id, accounts, data)?
        }
        discriminator::DlpDiscriminator::ProtocolClaimFees => {
            processor::process_protocol_claim_fees(program_id, accounts, data)?
        }
    }
    Ok(())
}
