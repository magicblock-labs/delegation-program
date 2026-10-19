use dlp_api::{
    error::DlpError,
    v2::{
        pda::{
            CHALLENGE_SEED, DELEGATION_CLUSTER_SEED, PENDING_COMMITMENT_SEED,
        },
        Challenge, DelegationCluster, PendingCommitment, RaiseChallengeArgs,
        CHALLENGE_OUTCOME_NONE, CHALLENGE_STATUS_AWAITING_REVEAL,
        PENDING_COMMITMENT_STATUS_ACTIVE,
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
    require, require_eq, require_eq_keys, require_ge, require_le,
    require_n_accounts, require_signer,
};

use crate::{
    processor::fast::utils::pda::{create_pda, AccountSpace},
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
/// 3: `[]`                 DelegationCluster PDA
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
        delegation_cluster,
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

    let pending_data = pending_commitment.try_borrow()?;
    let pending_state = PendingCommitment::decode(pending_data.as_ref())?;
    let clock = Clock::get()?;
    validate_pending_commitment(
        &pending_state,
        pending_commitment,
        &args,
        clock.slot,
    )?;

    let delegation_cluster_data = delegation_cluster.try_borrow()?;
    let delegation_cluster_state =
        DelegationCluster::decode(delegation_cluster_data.as_ref())?;
    validate_delegation_cluster(
        &delegation_cluster_state,
        delegation_cluster,
        &pending_state,
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
        .checked_add(pending_state.challenger_reveal_timeout_slots())
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
    drop(delegation_cluster_data);
    drop(pending_data);

    create_pda(
        challenge,
        &crate::fast::ID,
        AccountSpace::CurrentRent(Challenge::DATA_LEN),
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
    let mut pending_state = PendingCommitment::decode_mut(pending_commitment)?;
    *pending_state.status_mut()? =
        PENDING_COMMITMENT_STATUS_AWAITING_CHALLENGER_REVEAL;
    pending_state
        .active_challenge_mut()?
        .set(Some(challenge_address))?;

    Ok(())
}

fn validate_delegation_cluster(
    delegation_cluster: &dlp_api::v2::DelegationClusterView<'_>,
    delegation_cluster_account: &AccountView,
    pending: &dlp_api::v2::PendingCommitmentView<'_>,
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
    require_ge!(
        args.stake_lamports(),
        pending_commitment.min_challenger_stake(),
        ProgramError::InvalidInstructionData
    );

    Ok(())
}
