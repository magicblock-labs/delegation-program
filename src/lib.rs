#![allow(unexpected_cfgs)]

extern crate dlp_api;
pub use ::solana_program;
#[allow(unused_imports)]
pub(crate) use dlp_api::diff;
#[allow(unused_imports)]
pub(crate) use dlp_api::{
    account_size_class, args, compact, consts, discriminator, error, pda,
    pod_view, requires, state,
};
#[allow(unused_imports)]
pub(crate) use dlp_api::{
    commit_record_seeds_from_delegated_account,
    commit_state_seeds_from_delegated_account,
    delegate_buffer_seeds_from_delegated_account,
    delegation_metadata_seeds_from_delegated_account,
    delegation_record_seeds_from_delegated_account,
    ephemeral_balance_seeds_from_payer, fees_vault_seeds,
    magic_fee_vault_seeds_from_validator, program_config_seeds_from_program_id,
    undelegate_buffer_seeds_from_delegated_account,
    undelegation_request_seeds_from_delegated_account,
    validator_fees_vault_seeds_from_validator,
};
pub use dlp_api::{id, ID};
#[allow(unused_imports)]
pub(crate) use dlp_api::{
    require, require_eq, require_eq_keys, require_ge, require_gt,
    require_initialized_pda, require_initialized_pda_fast, require_le,
    require_lt, require_n_accounts, require_n_accounts_with_optionals,
    require_owned_by, require_pda, require_signer, require_some,
};
#[cfg(feature = "logging")]
use solana_program::msg;
#[cfg(feature = "processor")]
use {
    dlp_api::discriminator::DlpDiscriminator,
    solana_program::{
        account_info::AccountInfo, entrypoint::ProgramResult,
        program_error::ProgramError, pubkey::Pubkey,
    },
};

#[cfg(feature = "processor")]
mod processor;

#[allow(unused_imports)]
pub(crate) use diff::*;

#[cfg(feature = "log-cost")]
mod cu;

#[cfg(all(feature = "entrypoint", not(feature = "no-entrypoint")))]
mod entrypoint;

#[cfg(any(feature = "processor", feature = "pinocchio-rt"))]
pub(crate) mod fast {
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

#[cfg(feature = "processor")]
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
        DlpDiscriminator::DelegateWithActions => {
            Some(processor::fast::process_delegate_with_actions(
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
        DlpDiscriminator::RequestUndelegation => {
            Some(processor::fast::process_request_undelegation(
                program_id, accounts, data,
            ))
        }
        DlpDiscriminator::UndelegateWithRollbackAfterTimeout => Some(
            processor::fast::process_undelegate_with_rollback_after_timeout(
                program_id, accounts, data,
            ),
        ),
        _ => None,
    }
}

#[cfg(feature = "processor")]
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
