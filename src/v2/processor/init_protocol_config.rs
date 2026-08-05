use dlp_api::{
    compat::borsh::BorshDeserialize,
    pda::fees_vault_pda,
    v2::{
        pda::{PROTOCOL_CONFIG_SEED, VERIFIER_REGISTRY_SEED},
        InitProtocolConfigArgs, ProtocolConfig, VerifierRegistry,
    },
};
use solana_sdk_ids::system_program;

use crate::{
    processor::utils::{
        loaders::{load_program, load_signer, load_uninitialized_pda},
        pda::create_pda,
    },
    solana_program::{
        account_info::AccountInfo, entrypoint::ProgramResult,
        program_error::ProgramError, pubkey::Pubkey,
    },
};

/// Initialize the global v2 protocol config accounts.
///
/// Accounts:
/// 0: `[signer, writable]` authority that pays rent and controls v2 config
/// 1: `[writable]`         ProtocolConfig PDA
/// 2: `[writable]`         VerifierRegistry PDA
/// 3: `[]`                 system program
pub fn process_init_protocol_config(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let args = InitProtocolConfigArgs::try_from_slice(data)?;
    validate_args(&args)?;

    let [authority, protocol_config, verifier_registry, system_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    load_signer(authority, "authority")?;
    load_program(system_program, system_program::id(), "system program")?;

    let protocol_config_bump = load_uninitialized_pda(
        protocol_config,
        &[PROTOCOL_CONFIG_SEED],
        &crate::id(),
        true,
        "protocol config",
    )?;
    let verifier_registry_bump = load_uninitialized_pda(
        verifier_registry,
        &[VERIFIER_REGISTRY_SEED],
        &crate::id(),
        true,
        "verifier registry",
    )?;

    create_pda(
        protocol_config,
        &crate::id(),
        ProtocolConfig::SPACE,
        &[PROTOCOL_CONFIG_SEED],
        protocol_config_bump,
        system_program,
        authority,
    )?;

    let verifier_registry_state = VerifierRegistry::default();
    create_pda(
        verifier_registry,
        &crate::id(),
        verifier_registry_state.size_with_discriminator(),
        &[VERIFIER_REGISTRY_SEED],
        verifier_registry_bump,
        system_program,
        authority,
    )?;

    let protocol_config_state = ProtocolConfig {
        authority: *authority.key,
        paused: false,
        vrf_program: args.vrf_program,
        vrf_config: args.vrf_config,
        resolver: args.resolver,
        protocol_fee_vault: fees_vault_pda(),
        min_operator_bond: args.min_operator_bond,
        min_verifier_bond: args.min_verifier_bond,
        min_challenger_stake: args.min_challenger_stake,
        challenge_window_slots: args.challenge_window_slots,
        operator_response_timeout_slots: args.operator_response_timeout_slots,
        challenger_reveal_timeout_slots: args.challenger_reveal_timeout_slots,
        payout_timelock_slots: args.payout_timelock_slots,
        selected_verifier_count: args.selected_verifier_count,
        approval_threshold: args.approval_threshold,
        max_window_extensions: args.max_window_extensions,
        match_penalty_bps: args.match_penalty_bps,
    };

    let mut protocol_config_data = protocol_config.try_borrow_mut_data()?;
    protocol_config_state
        .to_bytes_with_discriminator(&mut protocol_config_data.as_mut())?;

    let mut verifier_registry_data = verifier_registry.try_borrow_mut_data()?;
    verifier_registry_state
        .to_bytes_with_discriminator(&mut verifier_registry_data.as_mut())?;

    Ok(())
}

fn validate_args(args: &InitProtocolConfigArgs) -> ProgramResult {
    if args.vrf_program == Pubkey::default()
        || args.vrf_config == Pubkey::default()
        || args.resolver == Pubkey::default()
        || args.min_operator_bond == 0
        || args.min_verifier_bond == 0
        || args.min_challenger_stake == 0
        || args.challenge_window_slots == 0
        || args.operator_response_timeout_slots == 0
        || args.challenger_reveal_timeout_slots == 0
        || args.payout_timelock_slots == 0
        || args.selected_verifier_count == 0
        || args.approval_threshold == 0
        || args.approval_threshold > args.selected_verifier_count
        || args.match_penalty_bps > 10_000
    {
        return Err(ProgramError::InvalidInstructionData);
    }

    Ok(())
}
