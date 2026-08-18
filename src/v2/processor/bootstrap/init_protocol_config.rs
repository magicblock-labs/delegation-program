use dlp_api::{
    pda::fees_vault_pda,
    v2::{
        layout_error_to_program_error,
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
    processor::fast::{to_pinocchio_program_error, utils::pda::create_pda},
    requires::{
        require_program, require_signer, require_uninitialized_pda, ProgramCtx,
    },
    solana_program::pubkey::Pubkey,
};
use wheels::layout::{Decodable, Encodable};

/// Initialize the global v2 protocol config accounts.
///
/// Accounts:
/// 0: `[signer, writable]` authority that pays rent and controls v2 config
/// 1: `[writable]`         ProtocolConfig PDA
/// 2: `[writable]`         VerifierRegistry PDA
/// 3: `[]`                 system program
pub fn process_init_protocol_config(
    accounts: &[AccountView],
    data: &[u8],
) -> ProgramResult {
    let args = <InitProtocolConfigArgs as Decodable>::decode(data)
        .map_err(layout_error_to_program_error)
        .map_err(to_pinocchio_program_error)?;
    validate_args(&args)?;

    let [authority, protocol_config, verifier_registry, system_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    require_signer(authority, "authority")?;
    require_program(system_program, &pinocchio_system::ID, "system program")?;

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
        verifier_registry_state
            .encoded_len()
            .map_err(layout_error_to_program_error)
            .map_err(to_pinocchio_program_error)?,
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
    protocol_config_state
        .encode_to(protocol_config_data.as_mut())
        .map_err(layout_error_to_program_error)
        .map_err(to_pinocchio_program_error)?;

    let mut verifier_registry_data = verifier_registry.try_borrow_mut()?;
    verifier_registry_state
        .encode_to(verifier_registry_data.as_mut())
        .map_err(layout_error_to_program_error)
        .map_err(to_pinocchio_program_error)?;

    Ok(())
}

fn validate_args(args: &InitProtocolConfigArgsView<'_>) -> ProgramResult {
    if args.vrf_program() == &Pubkey::default()
        || args.vrf_config() == &Pubkey::default()
        || args.resolver() == &Pubkey::default()
        || args.min_operator_bond() == 0
        || args.min_verifier_bond() == 0
        || args.min_challenger_stake() == 0
        || args.challenge_window_slots() == 0
        || args.operator_response_timeout_slots() == 0
        || args.challenger_reveal_timeout_slots() == 0
        || args.payout_timelock_slots() == 0
        || args.selected_verifier_count() == 0
        || args.approval_threshold() == 0
        || args.approval_threshold() > args.selected_verifier_count()
        || args.match_penalty_bps() > 10_000
    {
        return Err(ProgramError::InvalidInstructionData);
    }

    Ok(())
}
