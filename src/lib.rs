#![allow(unexpected_cfgs)] // silence clippy for target_os solana and other solana program custom features

use crate::discriminator::DlpDiscriminator;
use pinocchio_log::log;
use solana_program::account_info::AccountInfo;
use solana_program::entrypoint::ProgramResult;
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;
use solana_program::{declare_id, msg};

pub mod args;
pub mod consts;
mod discriminator;
pub mod error;
pub mod instruction_builder;
pub mod pda;
mod processor;
pub mod state;

#[cfg(feature = "log-cost")]
mod cu;
#[cfg(not(feature = "no-entrypoint"))]
mod entrypoint;

declare_id!("DELeGGvXpWV2fqJUhqcF5ZSYMS4JTLjteaAMARRSaeSh");

pub mod fast {
    pinocchio_pubkey::declare_id!("DELeGGvXpWV2fqJUhqcF5ZSYMS4JTLjteaAMARRSaeSh");
}

#[cfg(all(not(feature = "no-entrypoint"), feature = "solana-security-txt"))]
solana_security_txt::security_txt! {
    name: "MagicBlock Delegation Program",
    project_url: "https://magicblock.gg",
    contacts: "email:dev@magicblock.gg,twitter:@magicblock",
    policy: "https://github.com/magicblock-labs/delegation-program/blob/master/LICENSE.md",
    preferred_languages: "en",
    source_code: "https://github.com/magicblock-labs/delegation-program"
}

pub fn fast_process_instruction(
    program_id: &pinocchio::pubkey::Pubkey,
    accounts: &[pinocchio::account_info::AccountInfo],
    data: &[u8],
) -> Option<pinocchio::ProgramResult> {
    if data.len() < 8 {
        return Some(Err(
            pinocchio::program_error::ProgramError::InvalidInstructionData,
        ));
    }

    let (discriminator_bytes, data) = data.split_at(8);

    let discriminator = match DlpDiscriminator::try_from(discriminator_bytes[0]) {
        Ok(discriminator) => discriminator,
        Err(_) => {
            log!("Failed to read and parse discriminator");
            return Some(Err(
                pinocchio::program_error::ProgramError::InvalidInstructionData,
            ));
        }
    };

    #[cfg(feature = "unit_test_config")]
    if matches!(
        discriminator,
        DlpDiscriminator::Delegate
            | DlpDiscriminator::CommitState
            | DlpDiscriminator::CommitStateFromBuffer
            | DlpDiscriminator::Finalize
            | DlpDiscriminator::Undelegate
    ) {
        msg!("Processing instruction: {:?}", discriminator);
    }

    match discriminator {
        DlpDiscriminator::Delegate => Some(processor::fast::process_delegate(
            program_id, accounts, data,
        )),
        DlpDiscriminator::CommitState => Some(processor::fast::process_commit_state(
            program_id, accounts, data,
        )),
        DlpDiscriminator::CommitStateFromBuffer => Some(
            processor::fast::process_commit_state_from_buffer(program_id, accounts, data),
        ),
        DlpDiscriminator::Finalize => Some(processor::fast::process_finalize(
            program_id, accounts, data,
        )),
        DlpDiscriminator::Undelegate => Some(processor::fast::process_undelegate(
            program_id, accounts, data,
        )),
        _ => None,
    }
}

pub fn slow_process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    if data.len() < 8 {
        return Err(ProgramError::InvalidInstructionData);
    }

    let (tag, data) = data.split_at(8);
    let ix = DlpDiscriminator::try_from(tag[0]).or(Err(ProgramError::InvalidInstructionData))?;

    msg!("Processing instruction: {:?}", ix);
    match ix {
        DlpDiscriminator::InitValidatorFeesVault => {
            processor::process_init_validator_fees_vault(program_id, accounts, data)?
        }
        DlpDiscriminator::InitProtocolFeesVault => {
            processor::process_init_protocol_fees_vault(program_id, accounts, data)?
        }
        DlpDiscriminator::ValidatorClaimFees => {
            processor::process_validator_claim_fees(program_id, accounts, data)?
        }
        DlpDiscriminator::WhitelistValidatorForProgram => {
            processor::process_whitelist_validator_for_program(program_id, accounts, data)?
        }
        DlpDiscriminator::TopUpEphemeralBalance => {
            processor::process_top_up_ephemeral_balance(program_id, accounts, data)?
        }
        DlpDiscriminator::DelegateEphemeralBalance => {
            processor::process_delegate_ephemeral_balance(program_id, accounts, data)?
        }
        DlpDiscriminator::CloseEphemeralBalance => {
            processor::process_close_ephemeral_balance(program_id, accounts, data)?
        }
        DlpDiscriminator::ProtocolClaimFees => {
            processor::process_protocol_claim_fees(program_id, accounts, data)?
        }
        DlpDiscriminator::CloseValidatorFeesVault => {
            processor::process_close_validator_fees_vault(program_id, accounts, data)?
        }
        DlpDiscriminator::CallHandler => {
            processor::process_call_handler(program_id, accounts, data)?
        }
        _ => {
            log!("PANIC: Instruction must be processed by fast_process_instruction");
            return Err(ProgramError::InvalidInstructionData);
        }
    }
    Ok(())
}
