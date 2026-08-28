use dlp_api::{
    error::DlpError,
    v2::{
        pda::{CHALLENGE_SEED, PENDING_COMMITMENT_SEED, PROTOCOL_CONFIG_SEED},
        Challenge, PendingCommitment, ProtocolConfig, RaiseChallengeArgs,
        SelectedVerifier, CHALLENGE_OUTCOME_NONE,
        CHALLENGE_STATUS_AWAITING_REVEAL, PENDING_COMMITMENT_STATUS_ACTIVE,
        PENDING_COMMITMENT_STATUS_AWAITING_CHALLENGER_REVEAL,
    },
};
use pinocchio::{
    cpi::{Seed, Signer},
    error::ProgramError,
    sysvars::{clock::Clock, Sysvar},
    AccountView, ProgramResult,
};
use pinocchio_system::instructions as system;
use wheels::{
    layout::{Decodable, Encodable},
    require_eq, require_ge, require_le, require_n_accounts, require_signer,
};

use crate::{
    processor::fast::utils::pda::create_pda,
    requires::{
        require_initialized_pda, require_owned_pda, require_uninitialized_pda,
        StandardCtx,
    },
};

/// Raise a hash-only challenge against one v2 pending commitment.
///
/// Accounts:
/// 0: `[signer, writable]` challenger identity and stake payer
/// 1: `[writable]`         Challenge PDA
/// 2: `[writable]`         PendingCommitment PDA
/// 3: `[]`                 ProtocolConfig PDA
/// 4: `[]`                 system program, required by system CPI
#[inline(never)]
pub fn process_raise_challenge(
    accounts: &[AccountView],
    data: &[u8],
) -> ProgramResult {
    let [
        challenger, // force multi-line
        challenge,
        pending_commitment,
        protocol_config,
        _system_program,
    ] = require_n_accounts!(accounts, 5);

    let args = RaiseChallengeArgs::decode(data)?;

    require_signer!(challenger);
    if !challenger.is_writable() {
        return Err(ProgramError::Immutable);
    }
    require_owned_pda(
        pending_commitment,
        &crate::fast::ID,
        "pending commitment",
    )?;

    let protocol_config_data = protocol_config.try_borrow()?;
    let protocol_config_state =
        load_protocol_config(protocol_config, protocol_config_data.as_ref())?;
    validate_protocol_config(&protocol_config_state, &args)?;

    let pending_data = pending_commitment.try_borrow()?;
    let pending_state = PendingCommitment::decode(pending_data.as_ref())?;
    let clock = Clock::get()?;
    validate_pending_commitment(
        &pending_state,
        pending_commitment,
        &args,
        clock.slot,
    )?;

    let pending_account_pubkey = *pending_state.account_pubkey();
    let commit_id_bytes = pending_state.commit_id().to_le_bytes();
    let challenge_bump = require_uninitialized_pda(
        challenge,
        &[
            CHALLENGE_SEED,
            pending_account_pubkey.as_ref(),
            &commit_id_bytes,
            challenger.address().as_ref(),
        ],
        &crate::fast::ID,
        true,
        StandardCtx::new("challenge"),
    )?;

    let challenge_address = challenge.address().clone();
    let reveal_deadline_slot = clock
        .slot
        .checked_add(protocol_config_state.challenger_reveal_timeout_slots())
        .ok_or(DlpError::Overflow)?;

    let challenge_state = Challenge {
        discriminator: Challenge::DISCRIMINATOR,
        status: CHALLENGE_STATUS_AWAITING_REVEAL,
        outcome: CHALLENGE_OUTCOME_NONE,
        _pad_after_outcome: [0; 6],
        pending_commitment: pending_commitment.address().clone(),
        challenger_identity: challenger.address().clone(),
        state_commitment_hash: *args.state_commitment_hash(),
        challenge_hash: *args.challenge_hash(),
        challenger_lamports: 0,
        challenger_owner: Default::default(),
        challenger_data_hash: [0; 32],
        challenger_state_buffer: Default::default(),
        challenger_stake_lamports: args.stake_lamports(),
        raised_slot: clock.slot,
        reveal_deadline_slot,
    };
    let updated_pending =
        copy_pending_with_challenge(&pending_state, challenge_address);
    drop(pending_data);
    drop(protocol_config_data);

    create_pda(
        challenge,
        &crate::fast::ID,
        Challenge::DATA_LEN,
        &[Signer::from(&[
            Seed::from(CHALLENGE_SEED),
            Seed::from(pending_account_pubkey.as_ref()),
            Seed::from(&commit_id_bytes),
            Seed::from(challenger.address().as_ref()),
            Seed::from(&[challenge_bump]),
        ])],
        challenger,
    )?;

    system::Transfer {
        from: challenger,
        to: challenge,
        lamports: args.stake_lamports(),
    }
    .invoke()?;

    challenge_state.encode_to(challenge.try_borrow_mut()?.as_mut())?;
    updated_pending.encode_to(pending_commitment.try_borrow_mut()?.as_mut())?;

    Ok(())
}

fn load_protocol_config<'a>(
    protocol_config: &AccountView,
    data: &'a [u8],
) -> Result<dlp_api::v2::ProtocolConfigView<'a>, ProgramError> {
    require_initialized_pda(
        protocol_config,
        &[PROTOCOL_CONFIG_SEED],
        &crate::fast::ID,
        false,
        "protocol config",
    )?;

    Ok(ProtocolConfig::decode(data)?)
}

fn validate_protocol_config(
    protocol_config: &dlp_api::v2::ProtocolConfigView<'_>,
    args: &dlp_api::v2::RaiseChallengeArgsView<'_>,
) -> ProgramResult {
    if protocol_config.discriminator() != ProtocolConfig::DISCRIMINATOR {
        return Err(ProgramError::InvalidAccountData);
    }
    require_eq!(
        protocol_config.paused(),
        false,
        ProgramError::InvalidAccountData
    );
    require_ge!(
        args.stake_lamports(),
        protocol_config.min_challenger_stake(),
        ProgramError::InvalidInstructionData
    );

    Ok(())
}

fn validate_pending_commitment(
    pending_commitment: &dlp_api::v2::PendingCommitmentView<'_>,
    pending_commitment_account: &AccountView,
    args: &dlp_api::v2::RaiseChallengeArgsView<'_>,
    current_slot: u64,
) -> ProgramResult {
    if pending_commitment.discriminator() != PendingCommitment::DISCRIMINATOR {
        return Err(ProgramError::InvalidAccountData);
    }

    let commit_id_bytes = pending_commitment.commit_id().to_le_bytes();
    require_initialized_pda(
        pending_commitment_account,
        &[
            PENDING_COMMITMENT_SEED,
            pending_commitment.account_pubkey().as_ref(),
            &commit_id_bytes,
        ],
        &crate::fast::ID,
        true,
        "pending commitment",
    )?;

    require_eq!(
        pending_commitment.status(),
        PENDING_COMMITMENT_STATUS_ACTIVE,
        ProgramError::InvalidInstructionData
    );
    require_le!(
        current_slot,
        pending_commitment.challenge_window_end_slot(),
        ProgramError::InvalidInstructionData
    );
    require_eq!(
        pending_commitment.active_challenge().is_none(),
        true,
        ProgramError::InvalidInstructionData
    );
    require_eq!(
        pending_commitment.resolved_state_source().is_none(),
        true,
        ProgramError::InvalidInstructionData
    );
    require_eq!(
        args.state_commitment_hash(),
        pending_commitment.state_commitment_hash(),
        ProgramError::InvalidInstructionData
    );

    Ok(())
}

fn copy_pending_with_challenge(
    pending: &dlp_api::v2::PendingCommitmentView<'_>,
    challenge: dlp_api::compat::Pubkey,
) -> PendingCommitment {
    PendingCommitment {
        discriminator: PendingCommitment::DISCRIMINATOR,
        status: PENDING_COMMITMENT_STATUS_AWAITING_CHALLENGER_REVEAL,
        operator_identity: *pending.operator_identity(),
        operator_bond: *pending.operator_bond(),
        account_pubkey: *pending.account_pubkey(),
        commit_id: pending.commit_id(),
        delegation_record: *pending.delegation_record(),
        da_pointer_hash: *pending.da_pointer_hash(),
        account_state_hash: *pending.account_state_hash(),
        data_hash: *pending.data_hash(),
        lamports: pending.lamports(),
        owner: *pending.owner(),
        state_commitment_hash: *pending.state_commitment_hash(),
        verifier_registry: *pending.verifier_registry(),
        verifier_registry_revision: pending.verifier_registry_revision(),
        challenge_window_id: pending.challenge_window_id(),
        posted_slot: pending.posted_slot(),
        activation_slot: pending.activation_slot(),
        challenge_window_end_slot: pending.challenge_window_end_slot(),
        approval_count: pending.approval_count(),
        approval_threshold: pending.approval_threshold(),
        active_challenge: Some(challenge),
        resolved_state_source: pending.resolved_state_source(),
        er_slot: pending.er_slot(),
        _pad_before_selected_verifiers: [0; 7],
        selected_verifiers: pending
            .selected_verifiers()
            .iter()
            .map(|verifier| SelectedVerifier {
                verifier_identity: *verifier.verifier_identity(),
                approved: verifier.approved(),
                _pad_after_approved: [0; 7],
            })
            .collect(),
    }
}
