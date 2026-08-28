use dlp_api::{
    error::DlpError,
    v2::{
        pda::{
            CHALLENGE_SEED, PENDING_COMMITMENT_SEED, PROTOCOL_CONFIG_SEED,
            STATE_BUFFER_SEED,
        },
        Challenge, ChallengerRevealArgs, PendingCommitment, ProtocolConfig,
        SelectedVerifier, StateBuffer, CHALLENGE_OUTCOME_INVALID_REVEAL,
        CHALLENGE_OUTCOME_MATCHING_STATE_CHALLENGER_PENALIZED,
        CHALLENGE_OUTCOME_NONE, CHALLENGE_STATUS_AWAITING_RESOLVER,
        CHALLENGE_STATUS_AWAITING_REVEAL, CHALLENGE_STATUS_TERMINAL,
        PENDING_COMMITMENT_STATUS_ACTIVE,
        PENDING_COMMITMENT_STATUS_AWAITING_CHALLENGER_REVEAL,
        PENDING_COMMITMENT_STATUS_AWAITING_DISPUTE_RESOLUTION,
    },
};
use pinocchio::{
    error::ProgramError,
    sysvars::{clock::Clock, Sysvar},
    AccountView, ProgramResult,
};
use wheels::{
    layout::{Decodable, Encodable},
    require_eq, require_eq_keys, require_le, require_n_accounts,
    require_signer,
};

use crate::requires::{require_initialized_pda, require_owned_pda};

/// Reveal challenger state for one v2 challenge.
///
/// Accounts:
/// 0: `[signer, writable]` challenger identity and refund account
/// 1: `[writable]`         Challenge PDA
/// 2: `[writable]`         PendingCommitment PDA
/// 3: `[]`                 operator StateBuffer PDA
/// 4: `[]`                 challenger StateBuffer PDA
/// 5: `[]`                 ProtocolConfig PDA
/// 6: `[writable]`         protocol fee vault
#[inline(never)]
pub fn process_challenger_reveal(
    accounts: &[AccountView],
    data: &[u8],
) -> ProgramResult {
    let [
        challenger, // force multi-line
        challenge,
        pending_commitment,
        operator_state_buffer,
        challenger_state_buffer,
        protocol_config,
        protocol_fee_vault,
    ] = require_n_accounts!(accounts, 7);

    let args = ChallengerRevealArgs::decode(data)?;

    require_signer!(challenger);
    if !challenger.is_writable()
        || !challenge.is_writable()
        || !pending_commitment.is_writable()
        || !protocol_fee_vault.is_writable()
    {
        return Err(ProgramError::Immutable);
    }

    require_owned_pda(challenge, &crate::fast::ID, "challenge")?;
    require_owned_pda(
        pending_commitment,
        &crate::fast::ID,
        "pending commitment",
    )?;

    let (configured_fee_vault, match_penalty_bps) =
        load_protocol_config(protocol_config)?;
    // CHECKPOINT: v2 treats the fee vault as the configured recipient, not as a
    // hard-coded PDA validation here. InitProtocolConfig currently sets it to
    // the legacy fees-vault PDA.
    require_eq_keys!(
        &configured_fee_vault,
        protocol_fee_vault.address(),
        ProgramError::InvalidAccountData
    );

    let mut pending = load_pending_commitment(pending_commitment)?;
    let mut challenge_state = load_challenge(challenge)?;
    let clock = Clock::get()?;

    validate_pending_commitment(&pending, pending_commitment, challenge)?;
    validate_challenge(
        &challenge_state,
        challenge,
        pending_commitment,
        challenger,
        &pending,
        clock.slot,
    )?;

    validate_state_buffer(
        operator_state_buffer,
        &pending.operator_identity,
        &pending.account_pubkey,
        pending.commit_id,
        &pending.data_hash,
    )?;

    let challenger_identity = challenger.address().clone();
    validate_state_buffer(
        challenger_state_buffer,
        &challenger_identity,
        &pending.account_pubkey,
        pending.commit_id,
        args.data_hash(),
    )?;

    // Keep the attempted opening in Challenge state even when the salted
    // challenge hash does not match.
    challenge_state.challenger_lamports = args.lamports();
    challenge_state.challenger_owner = *args.owner();
    challenge_state.challenger_data_hash = *args.data_hash();
    challenge_state.challenger_state_buffer =
        challenger_state_buffer.address().clone();

    let opened_state_hash =
        account_state_hash(args.lamports(), args.owner(), args.data_hash());
    let opened_challenge_hash =
        challenge_hash(&pending, &challenge_state, &args);

    // CHECKPOINT: invalid or matching reveals terminate only this challenge and
    // reopen the pending commitment. Challenge PDA rent remains until a later
    // close instruction, while resolver-worthy mismatches keep the stake locked.
    if opened_challenge_hash != challenge_state.challenge_hash {
        move_lamports(
            challenge,
            protocol_fee_vault,
            challenge_state.challenger_stake_lamports,
        )?;
        challenge_state.status = CHALLENGE_STATUS_TERMINAL;
        challenge_state.outcome = CHALLENGE_OUTCOME_INVALID_REVEAL;
        reopen_pending_commitment(&mut pending);
    } else if opened_state_hash == pending.account_state_hash {
        let penalty_lamports = match_penalty_lamports(
            challenge_state.challenger_stake_lamports,
            match_penalty_bps,
        )?;
        let refund_lamports = challenge_state
            .challenger_stake_lamports
            .checked_sub(penalty_lamports)
            .ok_or(DlpError::Overflow)?;

        move_lamports(challenge, protocol_fee_vault, penalty_lamports)?;
        move_lamports(challenge, challenger, refund_lamports)?;
        challenge_state.status = CHALLENGE_STATUS_TERMINAL;
        challenge_state.outcome =
            CHALLENGE_OUTCOME_MATCHING_STATE_CHALLENGER_PENALIZED;
        reopen_pending_commitment(&mut pending);
    } else {
        challenge_state.status = CHALLENGE_STATUS_AWAITING_RESOLVER;
        challenge_state.outcome = CHALLENGE_OUTCOME_NONE;
        pending.status = PENDING_COMMITMENT_STATUS_AWAITING_DISPUTE_RESOLUTION;
    }

    challenge_state.encode_to(challenge.try_borrow_mut()?.as_mut())?;
    pending.encode_to(pending_commitment.try_borrow_mut()?.as_mut())?;

    Ok(())
}

fn load_protocol_config(
    protocol_config: &AccountView,
) -> Result<(dlp_api::compat::Pubkey, u16), ProgramError> {
    require_initialized_pda(
        protocol_config,
        &[PROTOCOL_CONFIG_SEED],
        &crate::fast::ID,
        false,
        "protocol config",
    )?;

    let data = protocol_config.try_borrow()?;
    let state = ProtocolConfig::decode(data.as_ref())?;
    if state.discriminator() != ProtocolConfig::DISCRIMINATOR {
        return Err(ProgramError::InvalidAccountData);
    }
    require_eq!(state.paused(), false, ProgramError::InvalidAccountData);

    Ok((*state.protocol_fee_vault(), state.match_penalty_bps()))
}

fn load_pending_commitment(
    pending_commitment: &AccountView,
) -> Result<PendingCommitment, ProgramError> {
    let pending_data = pending_commitment.try_borrow()?;
    let pending_view = PendingCommitment::decode(pending_data.as_ref())?;

    if pending_view.discriminator() != PendingCommitment::DISCRIMINATOR {
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(PendingCommitment {
        discriminator: PendingCommitment::DISCRIMINATOR,
        status: pending_view.status(),
        operator_identity: *pending_view.operator_identity(),
        operator_bond: *pending_view.operator_bond(),
        account_pubkey: *pending_view.account_pubkey(),
        commit_id: pending_view.commit_id(),
        delegation_record: *pending_view.delegation_record(),
        da_pointer_hash: *pending_view.da_pointer_hash(),
        account_state_hash: *pending_view.account_state_hash(),
        data_hash: *pending_view.data_hash(),
        lamports: pending_view.lamports(),
        owner: *pending_view.owner(),
        state_commitment_hash: *pending_view.state_commitment_hash(),
        verifier_registry: *pending_view.verifier_registry(),
        verifier_registry_revision: pending_view.verifier_registry_revision(),
        challenge_window_id: pending_view.challenge_window_id(),
        posted_slot: pending_view.posted_slot(),
        activation_slot: pending_view.activation_slot(),
        challenge_window_end_slot: pending_view.challenge_window_end_slot(),
        approval_count: pending_view.approval_count(),
        approval_threshold: pending_view.approval_threshold(),
        active_challenge: pending_view.active_challenge().cloned(),
        resolved_state_source: pending_view.resolved_state_source(),
        er_slot: pending_view.er_slot(),
        _pad_before_selected_verifiers: [0; 7],
        selected_verifiers: pending_view
            .selected_verifiers()
            .iter()
            .map(|verifier| SelectedVerifier {
                verifier_identity: *verifier.verifier_identity(),
                approved: verifier.approved(),
                _pad_after_approved: [0; 7],
            })
            .collect(),
    })
}

fn load_challenge(challenge: &AccountView) -> Result<Challenge, ProgramError> {
    let challenge_data = challenge.try_borrow()?;
    let challenge_view = Challenge::decode(challenge_data.as_ref())?;

    if challenge_view.discriminator() != Challenge::DISCRIMINATOR {
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(Challenge {
        discriminator: Challenge::DISCRIMINATOR,
        status: challenge_view.status(),
        outcome: challenge_view.outcome(),
        _pad_after_outcome: [0; 6],
        pending_commitment: *challenge_view.pending_commitment(),
        challenger_identity: *challenge_view.challenger_identity(),
        state_commitment_hash: *challenge_view.state_commitment_hash(),
        challenge_hash: *challenge_view.challenge_hash(),
        challenger_lamports: challenge_view.challenger_lamports(),
        challenger_owner: *challenge_view.challenger_owner(),
        challenger_data_hash: *challenge_view.challenger_data_hash(),
        challenger_state_buffer: *challenge_view.challenger_state_buffer(),
        challenger_stake_lamports: challenge_view.challenger_stake_lamports(),
        raised_slot: challenge_view.raised_slot(),
        reveal_deadline_slot: challenge_view.reveal_deadline_slot(),
    })
}

fn validate_pending_commitment(
    pending: &PendingCommitment,
    pending_commitment: &AccountView,
    challenge: &AccountView,
) -> ProgramResult {
    let commit_id_bytes = pending.commit_id.to_le_bytes();
    require_initialized_pda(
        pending_commitment,
        &[
            PENDING_COMMITMENT_SEED,
            pending.account_pubkey.as_ref(),
            &commit_id_bytes,
        ],
        &crate::fast::ID,
        true,
        "pending commitment",
    )?;

    require_eq!(
        pending.status,
        PENDING_COMMITMENT_STATUS_AWAITING_CHALLENGER_REVEAL,
        ProgramError::InvalidInstructionData
    );
    let active_challenge = pending
        .active_challenge
        .as_ref()
        .ok_or(ProgramError::InvalidInstructionData)?;
    require_eq_keys!(
        active_challenge,
        challenge.address(),
        ProgramError::InvalidInstructionData
    );
    require_eq!(
        pending.resolved_state_source.is_none(),
        true,
        ProgramError::InvalidInstructionData
    );

    Ok(())
}

fn validate_challenge(
    challenge_state: &Challenge,
    challenge: &AccountView,
    pending_commitment: &AccountView,
    challenger: &AccountView,
    pending: &PendingCommitment,
    current_slot: u64,
) -> ProgramResult {
    let commit_id_bytes = pending.commit_id.to_le_bytes();
    require_initialized_pda(
        challenge,
        &[
            CHALLENGE_SEED,
            pending.account_pubkey.as_ref(),
            &commit_id_bytes,
            challenger.address().as_ref(),
        ],
        &crate::fast::ID,
        true,
        "challenge",
    )?;

    require_eq!(
        challenge_state.status,
        CHALLENGE_STATUS_AWAITING_REVEAL,
        ProgramError::InvalidInstructionData
    );
    require_eq!(
        challenge_state.outcome,
        CHALLENGE_OUTCOME_NONE,
        ProgramError::InvalidInstructionData
    );
    require_eq_keys!(
        &challenge_state.pending_commitment,
        pending_commitment.address(),
        ProgramError::InvalidInstructionData
    );
    require_eq_keys!(
        &challenge_state.challenger_identity,
        challenger.address(),
        DlpError::InvalidAuthority
    );
    require_eq!(
        &challenge_state.state_commitment_hash,
        &pending.state_commitment_hash,
        ProgramError::InvalidInstructionData
    );
    require_le!(
        current_slot,
        challenge_state.reveal_deadline_slot,
        ProgramError::InvalidInstructionData
    );

    Ok(())
}

fn validate_state_buffer(
    state_buffer: &AccountView,
    authority: &dlp_api::compat::Pubkey,
    account_pubkey: &dlp_api::compat::Pubkey,
    commit_id: u64,
    expected_data_hash: &[u8; 32],
) -> ProgramResult {
    let commit_id_bytes = commit_id.to_le_bytes();
    require_initialized_pda(
        state_buffer,
        &[
            STATE_BUFFER_SEED,
            account_pubkey.as_ref(),
            &commit_id_bytes,
            authority.as_ref(),
        ],
        &crate::fast::ID,
        false,
        "state buffer",
    )?;

    let data = state_buffer.try_borrow()?;
    if data.len() < StateBuffer::DATA_LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    let state = StateBuffer::decode(&data.as_ref()[..StateBuffer::DATA_LEN])?;

    if state.discriminator() != StateBuffer::DISCRIMINATOR {
        return Err(ProgramError::InvalidAccountData);
    }
    require_eq_keys!(state.authority(), authority, DlpError::InvalidAuthority);
    require_eq_keys!(
        state.account_pubkey(),
        account_pubkey,
        ProgramError::InvalidAccountData
    );
    require_eq!(
        state.commit_id(),
        commit_id,
        ProgramError::InvalidInstructionData
    );
    require_eq!(
        state.finalized(),
        true,
        ProgramError::InvalidInstructionData
    );
    require_eq!(
        state.written_len(),
        state.total_len(),
        ProgramError::InvalidInstructionData
    );
    require_eq!(
        state.data_hash(),
        expected_data_hash,
        ProgramError::InvalidInstructionData
    );

    let raw_end = StateBuffer::DATA_LEN
        .checked_add(state.total_len() as usize)
        .ok_or(DlpError::Overflow)?;
    if data.len() < raw_end {
        return Err(ProgramError::InvalidAccountData);
    }

    Ok(())
}

fn reopen_pending_commitment(pending: &mut PendingCommitment) {
    pending.status = PENDING_COMMITMENT_STATUS_ACTIVE;
    pending.active_challenge = None;
    pending.resolved_state_source = None;
}

fn match_penalty_lamports(
    stake_lamports: u64,
    match_penalty_bps: u16,
) -> Result<u64, ProgramError> {
    Ok(stake_lamports
        .checked_mul(match_penalty_bps as u64)
        .ok_or(DlpError::Overflow)?
        / 10_000)
}

fn move_lamports(
    from: &AccountView,
    to: &AccountView,
    amount: u64,
) -> ProgramResult {
    if amount == 0 {
        return Ok(());
    }

    let from_lamports = from
        .lamports()
        .checked_sub(amount)
        .ok_or(DlpError::Overflow)?;
    let to_lamports = to
        .lamports()
        .checked_add(amount)
        .ok_or(DlpError::Overflow)?;

    from.set_lamports(from_lamports);
    to.set_lamports(to_lamports);

    Ok(())
}

fn account_state_hash(
    lamports: u64,
    owner: &dlp_api::compat::Pubkey,
    data_hash: &[u8; 32],
) -> [u8; 32] {
    solana_sha256_hasher::hashv(&[
        b"magicblock.account_state.v1",
        &lamports.to_le_bytes(),
        owner.as_ref(),
        data_hash,
    ])
    .to_bytes()
}

fn challenge_hash(
    pending: &PendingCommitment,
    challenge: &Challenge,
    args: &dlp_api::v2::ChallengerRevealArgsView<'_>,
) -> [u8; 32] {
    solana_sha256_hasher::hashv(&[
        b"magicblock.challenge.v1",
        &challenge.state_commitment_hash,
        pending.operator_identity.as_ref(),
        challenge.challenger_identity.as_ref(),
        pending.account_pubkey.as_ref(),
        &pending.commit_id.to_le_bytes(),
        &args.lamports().to_le_bytes(),
        args.owner().as_ref(),
        args.data_hash(),
        args.salt(),
    ])
    .to_bytes()
}
