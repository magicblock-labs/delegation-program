use dlp_api::{
    compat::Pubkey,
    pda::fees_vault_pda,
    v2::{
        pda::{PROTOCOL_CONFIG_SEED, VERIFIER_REGISTRY_SEED},
        InitProtocolConfigArgs, InitProtocolConfigArgsView, ProtocolConfig,
        VerifierRegistry,
    },
};
use pinocchio::{
    cpi::{Seed, Signer},
    error::ProgramError,
    AccountView, ProgramResult,
};

use crate::{
    processor::fast::utils::pda::create_pda,
    requires::{require_uninitialized_pda, StandardCtx},
};
use wheels::{
    layout::{Decodable, Encodable},
    require_eq, require_le, require_n_accounts, require_ne, require_ne_keys,
    require_signer,
};

/// Initialize the global protocol config accounts.
///
/// Accounts:
/// 0: `[signer, writable]` authority that pays rent and controls v2 config
/// 1: `[writable]`         ProtocolConfig PDA
/// 2: `[writable]`         VerifierRegistry PDA
/// 3: `[]`                 system program, required by system CPI
#[inline(never)]
pub fn process_init_protocol_config(
    accounts: &[AccountView],
    data: &[u8],
) -> ProgramResult {
    let [
        authority, // force multi-line
        protocol_config,
        verifier_registry,
        _system_program,
    ] = require_n_accounts!(accounts, 4);

    require_signer!(authority);

    let args = InitProtocolConfigArgs::decode(data)?;

    validate_protocol_config_args(&args)?;

    let protocol_config_bump = require_uninitialized_pda(
        protocol_config,
        &[PROTOCOL_CONFIG_SEED],
        &crate::fast::ID,
        true,
        StandardCtx::new("protocol config"),
    )?;
    let verifier_registry_bump = require_uninitialized_pda(
        verifier_registry,
        &[VERIFIER_REGISTRY_SEED],
        &crate::fast::ID,
        true,
        StandardCtx::new("verifier registry"),
    )?;

    create_pda(
        protocol_config,
        &crate::fast::ID,
        ProtocolConfig::DATA_LEN,
        &[Signer::from(&[
            Seed::from(PROTOCOL_CONFIG_SEED),
            Seed::from(&[protocol_config_bump]),
        ])],
        authority,
    )?;

    create_pda(
        verifier_registry,
        &crate::fast::ID,
        VerifierRegistry::MIN_DATA_LEN,
        &[Signer::from(&[
            Seed::from(VERIFIER_REGISTRY_SEED),
            Seed::from(&[verifier_registry_bump]),
        ])],
        authority,
    )?;

    ProtocolConfig {
        discriminator: ProtocolConfig::DISCRIMINATOR,
        bump: protocol_config_bump,
        authority: authority.address().to_bytes().into(),
        paused: false,
        resolver: *args.resolver(),
        protocol_fee_vault: fees_vault_pda(),
        min_operator_bond: args.min_operator_bond(),
        min_verifier_bond: args.min_verifier_bond(),
        min_challenger_stake: args.min_challenger_stake(),
        challenge_window_slots: args.challenge_window_slots(),
        operator_response_timeout_slots: args.operator_response_timeout_slots(),
        challenger_reveal_timeout_slots: args.challenger_reveal_timeout_slots(),
        payout_timelock_slots: args.payout_timelock_slots(),
        verifiers_per_commitment: args.verifiers_per_commitment(),
        approval_threshold: args.approval_threshold(),
        max_window_extensions: args.max_window_extensions(),
        match_penalty_bps: args.match_penalty_bps(),
    }
    .encode_to(protocol_config.try_borrow_mut()?.as_mut())?;

    VerifierRegistry {
        discriminator: VerifierRegistry::DISCRIMINATOR,
        bump: verifier_registry_bump,
        registry_revision: 0,
        next_selection_index: 0,
        entries: Vec::new(),
    }
    .encode_to(verifier_registry.try_borrow_mut()?.as_mut())?;

    Ok(())
}

pub(super) fn validate_protocol_config_args(
    args: &InitProtocolConfigArgsView<'_>,
) -> ProgramResult {
    let default_pubkey = Pubkey::default();

    require_ne_keys!(
        args.resolver(),
        &default_pubkey,
        ProgramError::InvalidInstructionData
    );
    require_ne!(
        args.min_operator_bond(),
        0,
        ProgramError::InvalidInstructionData
    );
    require_ne!(
        args.min_verifier_bond(),
        0,
        ProgramError::InvalidInstructionData
    );
    require_ne!(
        args.min_challenger_stake(),
        0,
        ProgramError::InvalidInstructionData
    );
    require_ne!(
        args.challenge_window_slots(),
        0,
        ProgramError::InvalidInstructionData
    );
    require_ne!(
        args.operator_response_timeout_slots(),
        0,
        ProgramError::InvalidInstructionData
    );
    require_ne!(
        args.challenger_reveal_timeout_slots(),
        0,
        ProgramError::InvalidInstructionData
    );
    require_ne!(
        args.payout_timelock_slots(),
        0,
        ProgramError::InvalidInstructionData
    );
    require_eq!(
        args.verifiers_per_commitment(),
        1,
        ProgramError::InvalidInstructionData
    );
    require_eq!(
        args.approval_threshold(),
        1,
        ProgramError::InvalidInstructionData
    );
    require_le!(
        args.match_penalty_bps(),
        ProtocolConfig::MAX_MATCH_PENALTY_BPS,
        ProgramError::InvalidInstructionData
    );

    Ok(())
}
