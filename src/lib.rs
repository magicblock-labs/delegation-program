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

pub mod v2;

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

#[rustfmt::skip]
pub const GLOBAL_IX_TABLE: [v2::IxHandler; 256] = {
    // start from v2 table as initial value and then write the
    // other empty slots with v1 instructions so that in the
    // end, we'll have a single table containing all v1 and v2 instructions
    // that use pinocchio, though for non-pinocchio based instructions, we
    // use fallback using InstructionNotFound error returned by the default handler.
    let mut table = v2::IX_TABLE;

    use crate::discriminator::DlpDiscriminator::*;
    use crate::processor::fast::*;

    table[Delegate.index()]                  = process_delegate;
    table[DelegateWithAnyValidator.index()]  = process_delegate_with_any_validator;
    table[CommitState.index()]               = process_commit_state;
    table[CommitStateFromBuffer.index()]     = process_commit_state_from_buffer;
    table[CommitDiff.index()]                = process_commit_diff;
    table[CommitDiffFromBuffer.index()]      = process_commit_diff_from_buffer;
    table[CommitFinalize.index()]            = process_commit_finalize;
    table[CommitFinalizeFromBuffer.index()]  = process_commit_finalize_from_buffer;
    table[Finalize.index()]                  = process_finalize;
    table[Undelegate.index()]                = process_undelegate;
    table[UndelegateConfinedAccount.index()] = process_undelegate_confined_account;

    table
};

#[cfg(not(feature = "sdk"))]
pub fn fast_process_instruction(
    accounts: &[pinocchio::AccountView],
    ixdata: &[u8],
) -> Option<pinocchio::ProgramResult> {
    use crate::v2::process_hyper_commit_finalize;

    if ixdata.len() < 8 {
        return Some(Err(
            pinocchio::error::ProgramError::InvalidInstructionData,
        ));
    }

    let (discriminator_bytes, data) = unsafe { ixdata.split_at_unchecked(8) };

    #[cfg(feature = "logging")]
    msg!("Processing instruction: {:?}", discriminator);

    // 23 CU -- till here

    if true {
        //return Some(Ok(()));
        return Some(process_hyper_commit_finalize(accounts, data));
    } else {
        match GLOBAL_IX_TABLE[discriminator_bytes[0] as usize](accounts, data) {
            e @ Err(pinocchio::error::ProgramError::Custom(val)) => {
                use crate::error::DlpError;

                if val == DlpError::InstructionNotFound as u32 {
                    None
                } else {
                    Some(e)
                }
            }
            otherwise => Some(otherwise),
        }
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
