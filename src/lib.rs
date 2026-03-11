#![allow(unexpected_cfgs)]

// Exactly one of `sdk` or `program` must be enabled
#[cfg(all(feature = "sdk", feature = "program"))]
compile_error!(
    "Features `sdk` and `program` are mutually exclusive. Enable exactly one."
);

#[cfg(all(not(feature = "sdk"), not(feature = "program")))]
compile_error!(
    "Enable either `program` (default) or `sdk`. Building with neither is not supported."
);

use solana_program::declare_id;
#[cfg(feature = "logging")]
use solana_program::msg;
#[cfg(not(feature = "sdk"))]
use {
    crate::discriminator::DlpDiscriminator,
    solana_program::{
        account_info::AccountInfo, entrypoint::ProgramResult,
        program_error::ProgramError, pubkey::Pubkey,
    },
};

pub mod args;
pub mod consts;
mod discriminator;
pub mod error;
pub mod instruction_builder;
pub mod pda;
pub mod pod_view;
pub mod state;

mod account_size_class;

pub use account_size_class::*;

#[cfg(not(feature = "sdk"))]
mod diff;

#[cfg(not(feature = "sdk"))]
mod processor;

#[cfg(not(feature = "sdk"))]
pub use diff::*;
// re-export
#[cfg(not(feature = "sdk"))]
pub use rkyv;

#[cfg(feature = "log-cost")]
mod cu;

#[cfg(not(feature = "no-entrypoint"))]
mod entrypoint;

declare_id!("DELeGGvXpWV2fqJUhqcF5ZSYMS4JTLjteaAMARRSaeSh");

#[cfg(not(feature = "sdk"))]
pub mod fast {
    pinocchio::address::declare_id!(
        "DELeGGvXpWV2fqJUhqcF5ZSYMS4JTLjteaAMARRSaeSh"
    );
}

#[cfg(feature = "solana-security-txt")]
solana_security_txt::security_txt! {
    name: "MagicBlock Delegation Program",
    project_url: "https://magicblock.xyz",
    contacts: "email:dev@magicblock.gg,twitter:@magicblock",
    policy: "https://github.com/magicblock-labs/delegation-program/blob/master/LICENSE.md",
    preferred_languages: "en",
    source_code: "https://github.com/magicblock-labs/delegation-program"
}

#[cfg(not(feature = "sdk"))]
pub fn fast_process_instruction(
    program_id: &pinocchio::Address,
    accounts: &[pinocchio::AccountView],
    data: &[u8],
) -> Option<pinocchio::ProgramResult> {
    if data.len() < 8 {
        return Some(Err(
            pinocchio::error::ProgramError::InvalidInstructionData,
        ));
    }

    let (discriminator_bytes, data) = data.split_at(8);

    let discriminator = match DlpDiscriminator::try_from(discriminator_bytes[0])
    {
        Ok(discriminator) => discriminator,
        Err(_) => {
            pinocchio_log::log!("Failed to read and parse discriminator");
            return Some(Err(
                pinocchio::error::ProgramError::InvalidInstructionData,
            ));
        }
    };

    #[cfg(feature = "logging")]
    msg!("Processing instruction: {:?}", discriminator);

    match discriminator {
        DlpDiscriminator::Delegate => Some(processor::fast::process_delegate(
            program_id, accounts, data,
        )),
        DlpDiscriminator::DelegateWithAnyValidator => {
            Some(processor::fast::process_delegate_with_any_validator(
                program_id, accounts, data,
            ))
        }
        DlpDiscriminator::CommitState => Some(
            processor::fast::process_commit_state(program_id, accounts, data),
        ),
        DlpDiscriminator::CommitStateFromBuffer => {
            Some(processor::fast::process_commit_state_from_buffer(
                program_id, accounts, data,
            ))
        }
        DlpDiscriminator::CommitDiff => Some(
            processor::fast::process_commit_diff(program_id, accounts, data),
        ),
        DlpDiscriminator::CommitDiffFromBuffer => {
            Some(processor::fast::process_commit_diff_from_buffer(
                program_id, accounts, data,
            ))
        }
        DlpDiscriminator::CommitFinalize => {
            Some(processor::fast::process_commit_finalize(
                program_id, accounts, data,
            ))
        }
        DlpDiscriminator::CommitFinalizeFromBuffer => {
            Some(processor::fast::process_commit_finalize_from_buffer(
                program_id, accounts, data,
            ))
        }
        DlpDiscriminator::Finalize => Some(processor::fast::process_finalize(
            program_id, accounts, data,
        )),
        DlpDiscriminator::Undelegate => Some(
            processor::fast::process_undelegate(program_id, accounts, data),
        ),
        DlpDiscriminator::UndelegateConfinedAccount => {
            Some(processor::fast::process_undelegate_confined_account(
                program_id, accounts, data,
            ))
        }
        _ => None,
    }
}

#[cfg(not(feature = "sdk"))]
pub fn slow_process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    if data.len() < 8 {
        return Err(ProgramError::InvalidInstructionData);
    }

    let (tag, data) = data.split_at(8);
    let ix = DlpDiscriminator::try_from(tag[0])
        .or(Err(ProgramError::InvalidInstructionData))?;

    match ix {
        DlpDiscriminator::InitMagicFeeVault => {
            processor::process_init_magic_fee_vault(program_id, accounts, data)?
        }
        DlpDiscriminator::InitValidatorFeesVault => {
            processor::process_init_validator_fees_vault(
                program_id, accounts, data,
            )?
        }
        DlpDiscriminator::InitProtocolFeesVault => {
            processor::process_init_protocol_fees_vault(
                program_id, accounts, data,
            )?
        }
        DlpDiscriminator::ValidatorClaimFees => {
            processor::process_validator_claim_fees(program_id, accounts, data)?
        }
        DlpDiscriminator::WhitelistValidatorForProgram => {
            processor::process_whitelist_validator_for_program(
                program_id, accounts, data,
            )?
        }
        DlpDiscriminator::TopUpEphemeralBalance => {
            processor::process_top_up_ephemeral_balance(
                program_id, accounts, data,
            )?
        }
        DlpDiscriminator::DelegateEphemeralBalance => {
            processor::process_delegate_ephemeral_balance(
                program_id, accounts, data,
            )?
        }
        DlpDiscriminator::DelegateMagicFeeVault => {
            processor::process_delegate_magic_fee_vault(
                program_id, accounts, data,
            )?
        }
        DlpDiscriminator::CloseEphemeralBalance => {
            processor::process_close_ephemeral_balance(
                program_id, accounts, data,
            )?
        }
        DlpDiscriminator::ProtocolClaimFees => {
            processor::process_protocol_claim_fees(program_id, accounts, data)?
        }
        DlpDiscriminator::CloseValidatorFeesVault => {
            processor::process_close_validator_fees_vault(
                program_id, accounts, data,
            )?
        }
        DlpDiscriminator::CallHandler => {
            processor::process_call_handler(program_id, accounts, data)?
        }
        DlpDiscriminator::CallHandlerV2 => {
            processor::process_call_handler_v2(program_id, accounts, data)?
        }
        _ => {
            #[cfg(feature = "logging")]
            msg!("PANIC: Instruction must be processed by fast_process_instruction");
            return Err(ProgramError::InvalidInstructionData);
        }
    }
    Ok(())
}
