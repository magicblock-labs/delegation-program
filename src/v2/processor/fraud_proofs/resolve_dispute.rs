use dlp_api::{
    error::DlpError,
    v2::{
        pda::{
            CHALLENGE_SEED, OPERATOR_BOND_SEED, PENDING_COMMITMENT_SEED,
            PROTOCOL_CONFIG_SEED,
        },
        Challenge, OperatorBond, PendingCommitment, ProtocolConfig,
        ResolveDisputeArgs, SelectedVerifier,
        CHALLENGE_OUTCOME_CHALLENGER_CORRECT_OPERATOR_SLASHED,
        CHALLENGE_OUTCOME_NONE,
        CHALLENGE_OUTCOME_OPERATOR_CORRECT_CHALLENGER_SLASHED,
        CHALLENGE_STATUS_AWAITING_RESOLVER, CHALLENGE_STATUS_TERMINAL,
        DISPUTE_DECISION_CHALLENGER_STATE_CORRECT,
        DISPUTE_DECISION_OPERATOR_STATE_CORRECT, OPERATOR_STATUS_SLASHED,
        PENDING_COMMITMENT_STATUS_AWAITING_DISPUTE_RESOLUTION,
        PENDING_COMMITMENT_STATUS_RESOLVED_CHALLENGER,
        PENDING_COMMITMENT_STATUS_RESOLVED_OPERATOR,
        RESOLVED_STATE_SOURCE_CHALLENGER_REVEAL,
        RESOLVED_STATE_SOURCE_OPERATOR_COMMITMENT,
    },
};
use pinocchio::{error::ProgramError, AccountView, ProgramResult};
use wheels::{
    layout::{Decodable, Encodable},
    require_eq, require_eq_keys, require_n_accounts, require_signer,
};

use crate::requires::{require_initialized_pda, require_owned_pda};

/// Apply resolver decision for one v2 challenge.
///
/// Accounts:
/// 0: `[signer]`           resolver identity from ProtocolConfig
/// 1: `[writable]`         Challenge PDA
/// 2: `[writable]`         PendingCommitment PDA
/// 3: `[writable]`         OperatorBond PDA
/// 4: `[writable]`         challenger identity and refund account
/// 5: `[]`                 ProtocolConfig PDA
/// 6: `[writable]`         protocol fee vault
#[inline(never)]
pub fn process_resolve_dispute(
    accounts: &[AccountView],
    data: &[u8],
) -> ProgramResult {
    let [
        resolver, // force multi-line
        challenge,
        pending_commitment,
        operator_bond,
        challenger,
        protocol_config,
        protocol_fee_vault,
    ] = require_n_accounts!(accounts, 7);

    let args = ResolveDisputeArgs::decode(data)?;

    require_signer!(resolver);
    if !challenge.is_writable()
        || !pending_commitment.is_writable()
        || !operator_bond.is_writable()
        || !challenger.is_writable()
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
    require_owned_pda(operator_bond, &crate::fast::ID, "operator bond")?;

    let (configured_resolver, configured_fee_vault) =
        load_protocol_config(protocol_config)?;
    require_eq_keys!(
        &configured_resolver,
        resolver.address(),
        DlpError::InvalidAuthority
    );
    require_eq_keys!(
        &configured_fee_vault,
        protocol_fee_vault.address(),
        ProgramError::InvalidAccountData
    );

    let mut pending = load_pending_commitment(pending_commitment)?;
    let mut challenge_state = load_challenge(challenge)?;
    let mut operator_bond_state = load_operator_bond(operator_bond, &pending)?;

    validate_pending_commitment(&pending, pending_commitment, challenge)?;
    validate_challenge(
        &challenge_state,
        challenge,
        pending_commitment,
        challenger,
        &pending,
    )?;

    // CHECKPOINT: MVP dispute economics are intentionally immediate and simple:
    // operator-correct slashes the challenger stake to the fee vault;
    // challenger-correct refunds that stake and slashes the operator's full
    // recorded stake to the fee vault. No payout timelock is created here.
    match args.decision() {
        DISPUTE_DECISION_OPERATOR_STATE_CORRECT => {
            move_lamports(
                challenge,
                protocol_fee_vault,
                challenge_state.challenger_stake_lamports,
            )?;

            resolve_pending_commitment(
                &mut pending,
                PENDING_COMMITMENT_STATUS_RESOLVED_OPERATOR,
                RESOLVED_STATE_SOURCE_OPERATOR_COMMITMENT,
            );
            challenge_state.status = CHALLENGE_STATUS_TERMINAL;
            challenge_state.outcome =
                CHALLENGE_OUTCOME_OPERATOR_CORRECT_CHALLENGER_SLASHED;
        }
        DISPUTE_DECISION_CHALLENGER_STATE_CORRECT => {
            move_lamports(
                challenge,
                challenger,
                challenge_state.challenger_stake_lamports,
            )?;
            move_lamports(
                operator_bond,
                protocol_fee_vault,
                operator_bond_state.stake_lamports,
            )?;

            operator_bond_state.stake_lamports = 0;
            operator_bond_state.locked_lamports = 0;
            operator_bond_state.status = OPERATOR_STATUS_SLASHED;
            operator_bond_state.withdraw_requested_slot = None;

            resolve_pending_commitment(
                &mut pending,
                PENDING_COMMITMENT_STATUS_RESOLVED_CHALLENGER,
                RESOLVED_STATE_SOURCE_CHALLENGER_REVEAL,
            );
            challenge_state.status = CHALLENGE_STATUS_TERMINAL;
            challenge_state.outcome =
                CHALLENGE_OUTCOME_CHALLENGER_CORRECT_OPERATOR_SLASHED;
        }
        _ => return Err(ProgramError::InvalidInstructionData),
    }

    challenge_state.encode_to(challenge.try_borrow_mut()?.as_mut())?;
    pending.encode_to(pending_commitment.try_borrow_mut()?.as_mut())?;
    operator_bond_state.encode_to(operator_bond.try_borrow_mut()?.as_mut())?;

    Ok(())
}

fn load_protocol_config(
    protocol_config: &AccountView,
) -> Result<(dlp_api::compat::Pubkey, dlp_api::compat::Pubkey), ProgramError> {
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

    Ok((*state.resolver(), *state.protocol_fee_vault()))
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

fn load_operator_bond(
    operator_bond: &AccountView,
    pending: &PendingCommitment,
) -> Result<OperatorBond, ProgramError> {
    require_eq_keys!(
        &pending.operator_bond,
        operator_bond.address(),
        ProgramError::InvalidAccountData
    );
    require_initialized_pda(
        operator_bond,
        &[OPERATOR_BOND_SEED, pending.operator_identity.as_ref()],
        &crate::fast::ID,
        true,
        "operator bond",
    )?;

    let data = operator_bond.try_borrow()?;
    let state = OperatorBond::decode(data.as_ref())?;
    if state.discriminator() != OperatorBond::DISCRIMINATOR {
        return Err(ProgramError::InvalidAccountData);
    }
    require_eq_keys!(
        state.operator_identity(),
        &pending.operator_identity,
        DlpError::InvalidAuthority
    );

    Ok(OperatorBond {
        discriminator: OperatorBond::DISCRIMINATOR,
        operator_identity: *state.operator_identity(),
        stake_lamports: state.stake_lamports(),
        locked_lamports: state.locked_lamports(),
        status: state.status(),
        withdraw_requested_slot: state.withdraw_requested_slot(),
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
        PENDING_COMMITMENT_STATUS_AWAITING_DISPUTE_RESOLUTION,
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
        CHALLENGE_STATUS_AWAITING_RESOLVER,
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

    Ok(())
}

fn resolve_pending_commitment(
    pending: &mut PendingCommitment,
    status: u8,
    resolved_state_source: u8,
) {
    pending.status = status;
    pending.active_challenge = None;
    pending.resolved_state_source = Some(resolved_state_source);
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
