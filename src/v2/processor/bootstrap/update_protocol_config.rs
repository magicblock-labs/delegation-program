use dlp_api::{
    error::DlpError,
    v2::{pda::PROTOCOL_CONFIG_SEED, ProtocolConfig, UpdateProtocolConfigArgs},
};
use pinocchio::{
    address::Address, error::ProgramError, AccountView, ProgramResult,
};
use wheels::{
    layout::{Decodable, Encodable},
    require_eq_keys, require_n_accounts, require_signer,
};

use crate::requires::require_initialized_pda;

/// Update global v2 config for future commitments.
///
/// Accounts:
/// 0: `[signer]`   protocol authority
/// 1: `[writable]` ProtocolConfig PDA
#[inline(never)]
pub fn process_update_protocol_config(
    accounts: &[AccountView],
    data: &[u8],
) -> ProgramResult {
    let [
        authority, // force multi-line
        protocol_config,
    ] = require_n_accounts!(accounts, 2);

    let args = UpdateProtocolConfigArgs::decode(data)?;
    super::validate_protocol_config_args(&args)?;

    require_signer!(authority);
    require_initialized_pda(
        protocol_config,
        &[PROTOCOL_CONFIG_SEED],
        &crate::fast::ID,
        true,
        "protocol config",
    )?;

    let protocol_config_data = protocol_config.try_borrow()?;
    let current = ProtocolConfig::decode(protocol_config_data.as_ref())?;
    if current.discriminator() != ProtocolConfig::DISCRIMINATOR {
        return Err(ProgramError::InvalidAccountData);
    }
    require_eq_keys!(
        &Address::from(current.authority().to_bytes()),
        authority.address(),
        DlpError::InvalidAuthority
    );

    let updated = ProtocolConfig {
        discriminator: ProtocolConfig::DISCRIMINATOR,
        authority: *current.authority(),
        paused: current.paused(),
        resolver: *args.resolver(),
        protocol_fee_vault: *current.protocol_fee_vault(),
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
    };
    drop(protocol_config_data);

    updated.encode_to(protocol_config.try_borrow_mut()?.as_mut())?;

    Ok(())
}
