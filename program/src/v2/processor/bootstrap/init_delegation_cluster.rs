use delegation_program_interface::{
    compat::Pubkey,
    pda::fees_vault_pda,
    v2::{
        pda::{DELEGATION_CLUSTER_SEED, VERIFIER_REGISTRY_SEED},
        ChallengeConfig, ChallengeConfigView, DelegationCluster,
        InitDelegationClusterArgs, InitDelegationClusterArgsView,
        VerifierRegistry,
    },
};
use pinocchio::{
    cpi::{Seed, Signer},
    error::ProgramError,
    AccountView, ProgramResult,
};
use wheels::{
    layout::{Decodable, Encodable},
    require_eq, require_le, require_n_accounts_with_optionals, require_ne,
    require_ne_keys, require_signer,
};

use crate::{
    processor::fast::utils::pda::{create_pda, AccountSpace},
    requires::{require_uninitialized_pda, StandardCtx},
};

/// Initialize one delegation cluster and, when challenge-enabled, its verifier
/// registry.
///
/// Direct-mode accounts:
/// 0: `[signer, writable]` config authority and rent payer
/// 1: `[signer]`           stable cluster identity
/// 2: `[writable]`         DelegationCluster PDA
/// 3: `[]`                 system program, required by system CPI
/// 4: `[writable]`         VerifierRegistry PDA, optional in challenge mode
#[inline(never)]
pub fn process_init_delegation_cluster(
    accounts: &[AccountView],
    data: &[u8],
) -> ProgramResult {
    let (
        [config_authority, cluster_identity, delegation_cluster, _system_program],
        optional_accounts,
    ) = require_n_accounts_with_optionals!(accounts, 4);
    let verifier_registry = match optional_accounts.len() {
        0 => None,
        1 => Some(&optional_accounts[0]),
        _ => return Err(ProgramError::InvalidInstructionData),
    };

    let args = InitDelegationClusterArgs::decode(data)?;
    let challenge_config = args.challenge_config();
    require_eq!(
        verifier_registry.is_some(),
        challenge_config.is_some(),
        ProgramError::InvalidInstructionData
    );

    require_signer!(config_authority);
    require_signer!(cluster_identity);
    // CHECKPOINT: A signer-backed cluster identity prevents namespace claims
    // without introducing a global allocator. A registry could replace this.
    validate_init_args(&args, challenge_config.as_ref())?;

    let delegation_cluster_bump = require_uninitialized_pda(
        delegation_cluster,
        &[DELEGATION_CLUSTER_SEED, cluster_identity.address().as_ref()],
        &crate::fast::ID,
        true,
        StandardCtx::new("delegation cluster"),
    )?;

    let challenge_config = challenge_config.map(|config| ChallengeConfig {
        resolver: *config.resolver(),
        min_operator_bond: config.min_operator_bond(),
        min_verifier_bond: config.min_verifier_bond(),
        min_challenger_stake: config.min_challenger_stake(),
        challenge_window_slots: config.challenge_window_slots(),
        operator_response_timeout_slots: config
            .operator_response_timeout_slots(),
        challenger_reveal_timeout_slots: config
            .challenger_reveal_timeout_slots(),
        payout_timelock_slots: config.payout_timelock_slots(),
        verifiers_per_commitment: config.verifiers_per_commitment(),
        approval_threshold: config.approval_threshold(),
        max_window_extensions: config.max_window_extensions(),
        match_penalty_bps: config.match_penalty_bps(),
    });
    let delegation_cluster_len = if challenge_config.is_some() {
        DelegationCluster::MAX_DATA_LEN
    } else {
        DelegationCluster::MIN_DATA_LEN
    };

    create_pda(
        delegation_cluster,
        &crate::fast::ID,
        AccountSpace::CurrentRent(delegation_cluster_len),
        &[Signer::from(&[
            Seed::from(DELEGATION_CLUSTER_SEED),
            Seed::from(cluster_identity.address().as_ref()),
            Seed::from(&[delegation_cluster_bump]),
        ])],
        config_authority,
    )?;

    DelegationCluster {
        discriminator: DelegationCluster::DISCRIMINATOR,
        bump: delegation_cluster_bump,
        cluster_id: *cluster_identity.address(),
        config_authority: *config_authority.address(),
        validator_rotation_authority: *args.validator_rotation_authority(),
        current_validator: *args.current_validator(),
        paused: false,
        protocol_fee_vault: fees_vault_pda(),
        challenge_config,
    }
    .encode_to(delegation_cluster.try_borrow_mut()?.as_mut())?;

    if let Some(verifier_registry) = verifier_registry {
        let verifier_registry_bump = require_uninitialized_pda(
            verifier_registry,
            &[
                VERIFIER_REGISTRY_SEED,
                delegation_cluster.address().as_ref(),
            ],
            &crate::fast::ID,
            true,
            StandardCtx::new("verifier registry"),
        )?;

        create_pda(
            verifier_registry,
            &crate::fast::ID,
            AccountSpace::CurrentRent(VerifierRegistry::MIN_DATA_LEN),
            &[Signer::from(&[
                Seed::from(VERIFIER_REGISTRY_SEED),
                Seed::from(delegation_cluster.address().as_ref()),
                Seed::from(&[verifier_registry_bump]),
            ])],
            config_authority,
        )?;

        VerifierRegistry {
            discriminator: VerifierRegistry::DISCRIMINATOR,
            bump: verifier_registry_bump,
            delegation_cluster: *delegation_cluster.address(),
            next_selection_index: 0,
            entries: Vec::new(),
        }
        .encode_to(verifier_registry.try_borrow_mut()?.as_mut())?;
    }

    Ok(())
}

fn validate_init_args(
    args: &InitDelegationClusterArgsView<'_>,
    challenge_config: Option<&ChallengeConfigView<'_>>,
) -> ProgramResult {
    let default_pubkey = Pubkey::default();

    require_ne_keys!(
        args.current_validator(),
        &default_pubkey,
        ProgramError::InvalidInstructionData
    );
    require_ne_keys!(
        args.validator_rotation_authority(),
        &default_pubkey,
        ProgramError::InvalidInstructionData
    );

    if let Some(config) = challenge_config {
        validate_challenge_config(config)?;
    }

    Ok(())
}

pub(super) fn validate_challenge_config(
    config: &ChallengeConfigView<'_>,
) -> ProgramResult {
    require_ne_keys!(
        config.resolver(),
        &Pubkey::default(),
        ProgramError::InvalidInstructionData
    );
    require_ne!(
        config.min_operator_bond(),
        0,
        ProgramError::InvalidInstructionData
    );
    require_ne!(
        config.min_verifier_bond(),
        0,
        ProgramError::InvalidInstructionData
    );
    require_ne!(
        config.min_challenger_stake(),
        0,
        ProgramError::InvalidInstructionData
    );
    require_ne!(
        config.challenge_window_slots(),
        0,
        ProgramError::InvalidInstructionData
    );
    require_ne!(
        config.operator_response_timeout_slots(),
        0,
        ProgramError::InvalidInstructionData
    );
    require_ne!(
        config.challenger_reveal_timeout_slots(),
        0,
        ProgramError::InvalidInstructionData
    );
    require_ne!(
        config.payout_timelock_slots(),
        0,
        ProgramError::InvalidInstructionData
    );
    // The MVP selects one equally weighted verifier per commitment.
    require_eq!(
        config.verifiers_per_commitment(),
        1,
        ProgramError::InvalidInstructionData
    );
    require_eq!(
        config.approval_threshold(),
        1,
        ProgramError::InvalidInstructionData
    );
    require_le!(
        config.match_penalty_bps(),
        ChallengeConfig::MAX_MATCH_PENALTY_BPS,
        ProgramError::InvalidInstructionData
    );

    Ok(())
}
