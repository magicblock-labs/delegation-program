use dlp_api::{
    error::DlpError,
    v2::{pda::PROTOCOL_CONFIG_SEED, ProtocolConfig, UpdateProtocolConfigArgs},
};

use crate::{
    processor::utils::loaders::{load_initialized_pda, load_signer},
    solana_program::{
        account_info::AccountInfo, entrypoint::ProgramResult,
        program_error::ProgramError, pubkey::Pubkey,
    },
};

/// Update global v2 config for future commitments.
///
/// Accounts:
/// 0: `[signer]`   protocol authority
/// 1: `[writable]` ProtocolConfig PDA
pub fn process_update_protocol_config(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let args = UpdateProtocolConfigArgs::try_from_bytes(data)?;
    super::validate_protocol_config_args(&args)?;

    let [authority, protocol_config] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    load_signer(authority, "authority")?;
    load_initialized_pda(
        protocol_config,
        &[PROTOCOL_CONFIG_SEED],
        &crate::id(),
        true,
        "protocol config",
    )?;

    let protocol_config_data = protocol_config.try_borrow_data()?;
    let current = ProtocolConfig::try_from_bytes_with_discriminator(
        protocol_config_data.as_ref(),
    )?;
    drop(protocol_config_data);

    if current.authority != *authority.key {
        return Err(DlpError::InvalidAuthority.into());
    }

    // CHECKPOINT: authority rotation should be a separate instruction with
    // explicit safety rules, not an accidental side effect of config updates.
    //
    // CHECKPOINT: pause/unpause may deserve separate instructions so emergency
    // operations are easy to audit and hard to bundle with unrelated changes.
    let updated = ProtocolConfig {
        authority: current.authority,
        paused: current.paused,
        vrf_program: args.vrf_program,
        vrf_config: args.vrf_config,
        resolver: args.resolver,
        protocol_fee_vault: current.protocol_fee_vault,
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

    // CHECKPOINT: when `PendingCommitment` is implemented, every config value
    // that affects an active commitment must be copied into that commitment at
    // creation time. Updates here should only affect future commitments.
    let mut protocol_config_data = protocol_config.try_borrow_mut_data()?;
    updated.to_bytes_with_discriminator(protocol_config_data.as_mut())?;

    Ok(())
}
