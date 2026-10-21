use delegation_program_interface as dpi;
use dpi::{
    error::DlpError,
    v2::{
        pda::DELEGATION_CLUSTER_SEED, ChallengeConfig, DelegationCluster,
        UpdateDelegationClusterArgs,
    },
};
use pinocchio::{error::ProgramError, AccountView, ProgramResult};
use wheels::{
    layout::{Decodable, Encodable},
    require, require_eq_keys, require_n_accounts, require_signer, require_some,
};

use crate::requires::require_initialized_pda;

/// Update challenge policy for future commitments in one delegation cluster.
///
/// Accounts:
/// 0: `[signer]`   cluster config authority
/// 1: `[writable]` DelegationCluster PDA
#[inline(never)]
pub fn process_update_delegation_cluster(
    accounts: &[AccountView],
    data: &[u8],
) -> ProgramResult {
    let [config_authority, delegation_cluster] =
        require_n_accounts!(accounts, 2);

    let args = UpdateDelegationClusterArgs::decode(data)?;
    let challenge_config = require_some!(
        args.challenge_config(),
        ProgramError::InvalidInstructionData
    );
    super::validate_challenge_config(&challenge_config)?;

    require_signer!(config_authority);

    let delegation_cluster_data = delegation_cluster.try_borrow()?;
    let current = DelegationCluster::decode(delegation_cluster_data.as_ref())?;
    require_initialized_pda(
        delegation_cluster,
        &[DELEGATION_CLUSTER_SEED, current.cluster_id().as_ref()],
        &crate::fast::ID,
        true,
        "delegation cluster",
    )?;
    require!(
        current.discriminator() == DelegationCluster::DISCRIMINATOR,
        ProgramError::InvalidAccountData
    );
    require_eq_keys!(
        current.config_authority(),
        config_authority.address(),
        DlpError::InvalidAuthority
    );
    require_some!(current.challenge_config(), ProgramError::InvalidAccountData);

    let updated = DelegationCluster {
        discriminator: DelegationCluster::DISCRIMINATOR,
        bump: current.bump(),
        cluster_id: *current.cluster_id(),
        config_authority: *current.config_authority(),
        validator_rotation_authority: *current.validator_rotation_authority(),
        current_validator: *current.current_validator(),
        // CHECKPOINT: pause/unpause remains a separate design decision; this
        // instruction deliberately preserves the emergency-stop state.
        paused: current.paused(),
        protocol_fee_vault: *current.protocol_fee_vault(),
        challenge_config: Some(ChallengeConfig {
            resolver: *challenge_config.resolver(),
            min_operator_bond: challenge_config.min_operator_bond(),
            min_verifier_bond: challenge_config.min_verifier_bond(),
            min_challenger_stake: challenge_config.min_challenger_stake(),
            challenge_window_slots: challenge_config.challenge_window_slots(),
            operator_response_timeout_slots: challenge_config
                .operator_response_timeout_slots(),
            challenger_reveal_timeout_slots: challenge_config
                .challenger_reveal_timeout_slots(),
            payout_timelock_slots: challenge_config.payout_timelock_slots(),
            verifiers_per_commitment: challenge_config
                .verifiers_per_commitment(),
            approval_threshold: challenge_config.approval_threshold(),
            max_window_extensions: challenge_config.max_window_extensions(),
            match_penalty_bps: challenge_config.match_penalty_bps(),
        }),
    };
    drop(delegation_cluster_data);

    updated.encode_to(delegation_cluster.try_borrow_mut()?.as_mut())?;

    Ok(())
}
