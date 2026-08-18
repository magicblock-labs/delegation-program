use dlp_api::{
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
    requires::{require_uninitialized_pda, ProgramCtx},
    solana_program::pubkey::Pubkey,
};
use wheels::{
    layout::{Decodable, Encodable},
    require_le, require_n_accounts, require_ne, require_ne_keys,
    require_signer,
};

/// Initialize the global protocol config accounts.
///
/// Accounts:
/// 0: `[signer, writable]` authority that pays rent and controls v2 config
/// 1: `[writable]`         ProtocolConfig PDA
/// 2: `[writable]`         VerifierRegistry PDA
/// 3: `[]`                 system program, required by system CPI
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

    let args = InitProtocolConfigArgs::decode(data)?;

    validate_args(&args)?;

    require_signer!(authority);

    let protocol_config_bump = require_uninitialized_pda(
        protocol_config,
        &[PROTOCOL_CONFIG_SEED],
        &crate::fast::ID,
        true,
        ProgramCtx::new("protocol config"),
    )?;
    let verifier_registry_bump = require_uninitialized_pda(
        verifier_registry,
        &[VERIFIER_REGISTRY_SEED],
        &crate::fast::ID,
        true,
        ProgramCtx::new("verifier registry"),
    )?;

    create_pda(
        protocol_config,
        &crate::fast::ID,
        ProtocolConfig::SPACE,
        &[Signer::from(&[
            Seed::from(PROTOCOL_CONFIG_SEED),
            Seed::from(&[protocol_config_bump]),
        ])],
        authority,
    )?;

    let verifier_registry_state = VerifierRegistry::default();
    create_pda(
        verifier_registry,
        &crate::fast::ID,
        verifier_registry_state.encoded_len()?,
        &[Signer::from(&[
            Seed::from(VERIFIER_REGISTRY_SEED),
            Seed::from(&[verifier_registry_bump]),
        ])],
        authority,
    )?;

    let protocol_config_state = ProtocolConfig {
        discriminator: ProtocolConfig::DISCRIMINATOR,
        authority: authority.address().to_bytes().into(),
        paused: false,
        vrf_program: *args.vrf_program(),
        vrf_config: *args.vrf_config(),
        resolver: *args.resolver(),
        protocol_fee_vault: fees_vault_pda(),
        min_operator_bond: args.min_operator_bond(),
        min_verifier_bond: args.min_verifier_bond(),
        min_challenger_stake: args.min_challenger_stake(),
        challenge_window_slots: args.challenge_window_slots(),
        operator_response_timeout_slots: args.operator_response_timeout_slots(),
        challenger_reveal_timeout_slots: args.challenger_reveal_timeout_slots(),
        payout_timelock_slots: args.payout_timelock_slots(),
        selected_verifier_count: args.selected_verifier_count(),
        approval_threshold: args.approval_threshold(),
        max_window_extensions: args.max_window_extensions(),
        match_penalty_bps: args.match_penalty_bps(),
    };

    let mut protocol_config_data = protocol_config.try_borrow_mut()?;
    protocol_config_state.encode_to(protocol_config_data.as_mut())?;

    let mut verifier_registry_data = verifier_registry.try_borrow_mut()?;
    verifier_registry_state.encode_to(verifier_registry_data.as_mut())?;

    Ok(())
}

fn validate_args(args: &InitProtocolConfigArgsView<'_>) -> ProgramResult {
    let default_pubkey = Pubkey::default();

    require_ne_keys!(
        args.vrf_program(),
        &default_pubkey,
        ProgramError::InvalidInstructionData
    );
    require_ne_keys!(
        args.vrf_config(),
        &default_pubkey,
        ProgramError::InvalidInstructionData
    );
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
    require_ne!(
        args.selected_verifier_count(),
        0,
        ProgramError::InvalidInstructionData
    );
    require_ne!(
        args.approval_threshold(),
        0,
        ProgramError::InvalidInstructionData
    );
    require_le!(
        args.approval_threshold(),
        args.selected_verifier_count(),
        ProgramError::InvalidInstructionData
    );
    require_le!(
        args.match_penalty_bps(),
        10_000,
        ProgramError::InvalidInstructionData
    );

    Ok(())
}
