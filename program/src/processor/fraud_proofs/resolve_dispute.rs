use delegation_program_interface as dpi;
use dpi::{
    error::DlpError,
    pda::{
        CHALLENGE_SEED, DELEGATION_CLUSTER_SEED, OPERATOR_BOND_SEED,
        PENDING_COMMITMENT_SEED,
    },
    Challenge, ChallengeView, DelegationCluster, OperatorBond,
    OperatorBondView, OperatorStatus, PendingCommitment, PendingCommitmentView,
    ResolveDisputeArgs, CHALLENGE_OUTCOME_CHALLENGER_CORRECT_OPERATOR_SLASHED,
    CHALLENGE_OUTCOME_NONE,
    CHALLENGE_OUTCOME_OPERATOR_CORRECT_CHALLENGER_SLASHED,
    CHALLENGE_STATUS_AWAITING_RESOLVER, CHALLENGE_STATUS_TERMINAL,
    DISPUTE_DECISION_CHALLENGER_STATE_CORRECT,
    DISPUTE_DECISION_OPERATOR_STATE_CORRECT,
    PENDING_COMMITMENT_STATUS_AWAITING_DISPUTE_RESOLUTION,
    PENDING_COMMITMENT_STATUS_RESOLVED_CHALLENGER,
    PENDING_COMMITMENT_STATUS_RESOLVED_OPERATOR,
    RESOLVED_STATE_SOURCE_CHALLENGER_REVEAL,
    RESOLVED_STATE_SOURCE_OPERATOR_COMMITMENT,
};
use pinocchio::{error::ProgramError, AccountView, ProgramResult};
use wheels::{
    layout::Decodable, require, require_eq, require_eq_keys,
    require_n_accounts, require_signer,
};

use crate::requires::{require_initialized_pda, require_owned_pda};

/// Apply resolver decision for one v2 challenge.
///
/// Accounts:
/// 0: `[signer]`           resolver snapshotted in PendingCommitment
/// 1: `[writable]`         Challenge PDA
/// 2: `[writable]`         PendingCommitment PDA
/// 3: `[writable]`         OperatorBond PDA
/// 4: `[writable]`         challenger identity and refund account
/// 5: `[]`                 DelegationCluster PDA
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
        delegation_cluster,
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

    let pending_data = pending_commitment.try_borrow()?;
    let pending = PendingCommitment::decode(pending_data.as_ref())?;
    let challenge_data = challenge.try_borrow()?;
    let challenge_state = Challenge::decode(challenge_data.as_ref())?;
    let operator_bond_data = operator_bond.try_borrow()?;
    let operator_bond_state =
        OperatorBond::decode(operator_bond_data.as_ref())?;
    let delegation_cluster_data = delegation_cluster.try_borrow()?;
    let delegation_cluster_state =
        DelegationCluster::decode(delegation_cluster_data.as_ref())?;

    validate_pending_commitment(&pending, pending_commitment, challenge)?;
    validate_delegation_cluster(
        &delegation_cluster_state,
        delegation_cluster,
        &pending,
        resolver,
        protocol_fee_vault,
    )?;
    validate_operator_bond(&operator_bond_state, operator_bond, &pending)?;
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
    let (
        pending_status,
        resolved_state_source,
        challenge_outcome,
        slash_operator,
    ) = match args.decision() {
        DISPUTE_DECISION_OPERATOR_STATE_CORRECT => {
            move_lamports(
                challenge,
                protocol_fee_vault,
                challenge_state.challenger_stake_lamports(),
            )?;

            (
                PENDING_COMMITMENT_STATUS_RESOLVED_OPERATOR,
                RESOLVED_STATE_SOURCE_OPERATOR_COMMITMENT,
                CHALLENGE_OUTCOME_OPERATOR_CORRECT_CHALLENGER_SLASHED,
                false,
            )
        }
        DISPUTE_DECISION_CHALLENGER_STATE_CORRECT => {
            move_lamports(
                challenge,
                challenger,
                challenge_state.challenger_stake_lamports(),
            )?;
            move_lamports(
                operator_bond,
                protocol_fee_vault,
                operator_bond_state.stake_lamports(),
            )?;

            (
                PENDING_COMMITMENT_STATUS_RESOLVED_CHALLENGER,
                RESOLVED_STATE_SOURCE_CHALLENGER_REVEAL,
                CHALLENGE_OUTCOME_CHALLENGER_CORRECT_OPERATOR_SLASHED,
                true,
            )
        }
        _ => return Err(ProgramError::InvalidInstructionData),
    };

    drop(delegation_cluster_data);
    drop(operator_bond_data);
    drop(challenge_data);
    drop(pending_data);

    let mut challenge_state = Challenge::decode_mut(challenge)?;
    *challenge_state.status_mut()? = CHALLENGE_STATUS_TERMINAL;
    *challenge_state.outcome_mut()? = challenge_outcome;

    let mut pending = PendingCommitment::decode_mut(pending_commitment)?;
    *pending.status_mut()? = pending_status;
    pending.active_challenge_mut()?.set(None)?;
    pending
        .resolved_state_source_mut()?
        .set(Some(resolved_state_source))?;

    if slash_operator {
        let mut operator_bond = OperatorBond::decode_mut(operator_bond)?;
        operator_bond.stake_lamports_mut()?.set(0)?;
        operator_bond.locked_lamports_mut()?.set(0)?;
        *operator_bond.status_mut()? = OperatorStatus::Slashed.value();
        operator_bond.withdraw_requested_slot_mut()?.set(None)?;
    }

    Ok(())
}

fn validate_delegation_cluster(
    delegation_cluster: &dpi::DelegationClusterView<'_>,
    delegation_cluster_account: &AccountView,
    pending: &PendingCommitmentView<'_>,
    resolver: &AccountView,
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
        pending.resolver(),
        resolver.address(),
        DlpError::InvalidAuthority
    );
    require_eq_keys!(
        delegation_cluster.protocol_fee_vault(),
        protocol_fee_vault.address(),
        ProgramError::InvalidAccountData
    );

    Ok(())
}

fn validate_operator_bond(
    operator_bond_state: &OperatorBondView<'_>,
    operator_bond: &AccountView,
    pending: &PendingCommitmentView<'_>,
) -> ProgramResult {
    if operator_bond_state.discriminator() != OperatorBond::DISCRIMINATOR {
        return Err(ProgramError::InvalidAccountData);
    }
    require_eq_keys!(
        pending.operator_bond(),
        operator_bond.address(),
        ProgramError::InvalidAccountData
    );
    require_initialized_pda(
        operator_bond,
        &[OPERATOR_BOND_SEED, pending.delegation_cluster().as_ref()],
        &crate::fast::ID,
        true,
        "operator bond",
    )?;

    require_eq_keys!(
        operator_bond_state.delegation_cluster(),
        pending.delegation_cluster(),
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
        PENDING_COMMITMENT_STATUS_AWAITING_DISPUTE_RESOLUTION,
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
        CHALLENGE_STATUS_AWAITING_RESOLVER,
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

    Ok(())
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
