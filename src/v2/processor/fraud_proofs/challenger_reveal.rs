use dlp_api::{
    error::DlpError,
    v2::{
        pda::{
            CHALLENGE_SEED, DELEGATION_CLUSTER_SEED, PENDING_COMMITMENT_SEED,
            STATE_BUFFER_SEED,
        },
        Challenge, ChallengeView, ChallengerRevealArgs, DelegationCluster,
        PendingCommitment, PendingCommitmentView, StateBuffer,
        CHALLENGE_OUTCOME_INVALID_REVEAL,
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
    layout::Decodable, require, require_eq, require_eq_keys, require_le,
    require_n_accounts, require_signer,
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
/// 5: `[]`                 DelegationCluster PDA
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
        delegation_cluster,
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

    let pending_data = pending_commitment.try_borrow()?;
    let pending = PendingCommitment::decode(pending_data.as_ref())?;
    let challenge_data = challenge.try_borrow()?;
    let challenge_state = Challenge::decode(challenge_data.as_ref())?;
    let delegation_cluster_data = delegation_cluster.try_borrow()?;
    let delegation_cluster_state =
        DelegationCluster::decode(delegation_cluster_data.as_ref())?;
    let clock = Clock::get()?;

    validate_pending_commitment(&pending, pending_commitment, challenge)?;
    validate_delegation_cluster(
        &delegation_cluster_state,
        delegation_cluster,
        &pending,
        protocol_fee_vault,
    )?;
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
        pending.operator_identity(),
        pending.account_pubkey(),
        pending.commit_id(),
        pending.data_hash(),
    )?;

    let challenger_identity = challenger.address().clone();
    validate_state_buffer(
        challenger_state_buffer,
        &challenger_identity,
        pending.account_pubkey(),
        pending.commit_id(),
        args.data_hash(),
    )?;

    let opened_state_hash =
        account_state_hash(args.lamports(), args.owner(), args.data_hash());
    let opened_challenge_hash =
        challenge_hash(&pending, &challenge_state, &args);

    // CHECKPOINT: invalid or matching reveals terminate only this challenge and
    // reopen the pending commitment. Challenge PDA rent remains until a later
    // close instruction, while resolver-worthy mismatches keep the stake locked.
    let (challenge_status, challenge_outcome, pending_status, reopen_pending) =
        if opened_challenge_hash != *challenge_state.challenge_hash() {
            move_lamports(
                challenge,
                protocol_fee_vault,
                challenge_state.challenger_stake_lamports(),
            )?;
            (
                CHALLENGE_STATUS_TERMINAL,
                CHALLENGE_OUTCOME_INVALID_REVEAL,
                PENDING_COMMITMENT_STATUS_ACTIVE,
                true,
            )
        } else if opened_state_hash == *pending.account_state_hash() {
            let penalty_lamports = match_penalty_lamports(
                challenge_state.challenger_stake_lamports(),
                pending.match_penalty_bps(),
            )?;
            let refund_lamports = challenge_state
                .challenger_stake_lamports()
                .checked_sub(penalty_lamports)
                .ok_or(DlpError::Overflow)?;

            move_lamports(challenge, protocol_fee_vault, penalty_lamports)?;
            move_lamports(challenge, challenger, refund_lamports)?;
            (
                CHALLENGE_STATUS_TERMINAL,
                CHALLENGE_OUTCOME_MATCHING_STATE_CHALLENGER_PENALIZED,
                PENDING_COMMITMENT_STATUS_ACTIVE,
                true,
            )
        } else {
            (
                CHALLENGE_STATUS_AWAITING_RESOLVER,
                CHALLENGE_OUTCOME_NONE,
                PENDING_COMMITMENT_STATUS_AWAITING_DISPUTE_RESOLUTION,
                false,
            )
        };

    drop(delegation_cluster_data);
    drop(challenge_data);
    drop(pending_data);

    let mut challenge_state = Challenge::decode_mut(challenge)?;
    *challenge_state.challenger_lamports_mut()? = args.lamports();
    *challenge_state.challenger_owner_mut()? = *args.owner();
    *challenge_state.challenger_data_hash_mut()? = *args.data_hash();
    *challenge_state.challenger_state_buffer_mut()? =
        *challenger_state_buffer.address();
    *challenge_state.status_mut()? = challenge_status;
    *challenge_state.outcome_mut()? = challenge_outcome;

    let mut pending = PendingCommitment::decode_mut(pending_commitment)?;
    *pending.status_mut()? = pending_status;
    if reopen_pending {
        pending.active_challenge_mut()?.set(None)?;
        pending.resolved_state_source_mut()?.set(None)?;
    }

    Ok(())
}

fn validate_delegation_cluster(
    delegation_cluster: &dlp_api::v2::DelegationClusterView<'_>,
    delegation_cluster_account: &AccountView,
    pending: &PendingCommitmentView<'_>,
    protocol_fee_vault: &AccountView,
) -> ProgramResult {
    if delegation_cluster.discriminator() != DelegationCluster::DISCRIMINATOR {
        return Err(ProgramError::InvalidAccountData);
    }
    require_initialized_pda(
        delegation_cluster_account,
        &[
            DELEGATION_CLUSTER_SEED,
            delegation_cluster.cluster_id().as_ref(),
        ],
        &crate::fast::ID,
        false,
        "delegation cluster",
    )?;
    require_eq_keys!(
        delegation_cluster_account.address(),
        pending.delegation_cluster(),
        ProgramError::InvalidAccountData
    );
    require!(
        delegation_cluster.challenge_config().is_some(),
        ProgramError::InvalidAccountData
    );
    require_eq_keys!(
        delegation_cluster.protocol_fee_vault(),
        protocol_fee_vault.address(),
        ProgramError::InvalidAccountData
    );

    Ok(())
}

fn validate_pending_commitment(
    pending: &PendingCommitmentView<'_>,
    pending_commitment: &AccountView,
    challenge: &AccountView,
) -> ProgramResult {
    if pending.discriminator() != PendingCommitment::DISCRIMINATOR {
        return Err(ProgramError::InvalidAccountData);
    }

    let commit_id_bytes = pending.commit_id().to_le_bytes();
    require_initialized_pda(
        pending_commitment,
        &[
            PENDING_COMMITMENT_SEED,
            pending.account_pubkey().as_ref(),
            &commit_id_bytes,
        ],
        &crate::fast::ID,
        true,
        "pending commitment",
    )?;

    require_eq!(
        pending.status(),
        PENDING_COMMITMENT_STATUS_AWAITING_CHALLENGER_REVEAL,
        ProgramError::InvalidInstructionData
    );
    let active_challenge = pending
        .active_challenge()
        .ok_or(ProgramError::InvalidInstructionData)?;
    require_eq_keys!(
        active_challenge,
        challenge.address(),
        ProgramError::InvalidInstructionData
    );
    require_eq!(
        pending.resolved_state_source().is_none(),
        true,
        ProgramError::InvalidInstructionData
    );

    Ok(())
}

fn validate_challenge(
    challenge_state: &ChallengeView<'_>,
    challenge: &AccountView,
    pending_commitment: &AccountView,
    challenger: &AccountView,
    pending: &PendingCommitmentView<'_>,
    current_slot: u64,
) -> ProgramResult {
    if challenge_state.discriminator() != Challenge::DISCRIMINATOR {
        return Err(ProgramError::InvalidAccountData);
    }

    let commit_id_bytes = pending.commit_id().to_le_bytes();
    require_initialized_pda(
        challenge,
        &[
            CHALLENGE_SEED,
            pending.account_pubkey().as_ref(),
            &commit_id_bytes,
            challenger.address().as_ref(),
        ],
        &crate::fast::ID,
        true,
        "challenge",
    )?;

    require_eq!(
        challenge_state.status(),
        CHALLENGE_STATUS_AWAITING_REVEAL,
        ProgramError::InvalidInstructionData
    );
    require_eq!(
        challenge_state.outcome(),
        CHALLENGE_OUTCOME_NONE,
        ProgramError::InvalidInstructionData
    );
    require_eq_keys!(
        challenge_state.pending_commitment(),
        pending_commitment.address(),
        ProgramError::InvalidInstructionData
    );
    require_eq_keys!(
        challenge_state.challenger_identity(),
        challenger.address(),
        DlpError::InvalidAuthority
    );
    require_eq!(
        challenge_state.state_commitment_hash(),
        pending.state_commitment_hash(),
        ProgramError::InvalidInstructionData
    );
    require_le!(
        current_slot,
        challenge_state.reveal_deadline_slot(),
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
    let state = StateBuffer::decode(data.as_ref())?;

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
        state.payload().len(),
        state.total_len() as usize,
        ProgramError::InvalidInstructionData
    );
    require_eq!(
        state.data_hash(),
        expected_data_hash,
        ProgramError::InvalidInstructionData
    );

    Ok(())
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
    pending: &PendingCommitmentView<'_>,
    challenge: &ChallengeView<'_>,
    args: &dlp_api::v2::ChallengerRevealArgsView<'_>,
) -> [u8; 32] {
    solana_sha256_hasher::hashv(&[
        b"magicblock.challenge.v1",
        challenge.state_commitment_hash(),
        pending.operator_identity().as_ref(),
        challenge.challenger_identity().as_ref(),
        pending.account_pubkey().as_ref(),
        &pending.commit_id().to_le_bytes(),
        &args.lamports().to_le_bytes(),
        args.owner().as_ref(),
        args.data_hash(),
        args.salt(),
    ])
    .to_bytes()
}
