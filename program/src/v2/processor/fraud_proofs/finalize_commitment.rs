use delegation_program_interface::{
    error::DlpError,
    state::{DelegationMetadataFast, DelegationRecord, UndelegationRequester},
    v2::{
        pda::{
            CHALLENGE_SEED, DELEGATION_CLUSTER_SEED, PENDING_COMMITMENT_SEED,
            STATE_BUFFER_SEED,
        },
        Challenge, ChallengeView, DelegationCluster, PendingCommitment,
        PendingCommitmentView, StateBuffer,
        CHALLENGE_OUTCOME_CHALLENGER_CORRECT_OPERATOR_SLASHED,
        CHALLENGE_STATUS_TERMINAL, PENDING_COMMITMENT_STATUS_ACTIVE,
        PENDING_COMMITMENT_STATUS_FINALIZED,
        PENDING_COMMITMENT_STATUS_RESOLVED_CHALLENGER,
        PENDING_COMMITMENT_STATUS_RESOLVED_OPERATOR,
        RESOLVED_STATE_SOURCE_CHALLENGER_REVEAL,
        RESOLVED_STATE_SOURCE_OPERATOR_COMMITMENT,
    },
};
use pinocchio::{
    error::ProgramError,
    sysvars::{clock::Clock, rent::Rent, Sysvar},
    AccountView, ProgramResult,
};
use pinocchio_system::instructions as system;
use wheels::{
    layout::Decodable, require, require_eq, require_eq_keys, require_ge,
    require_gt, require_n_accounts_with_optionals, require_signer,
};

use crate::{
    processor::fast::{to_pinocchio_program_error, utils::LamportsOperation},
    requires::{
        require_initialized_delegation_metadata,
        require_initialized_delegation_record, require_initialized_pda,
        require_owned_pda,
    },
};

/// Finalize one approved v2 account-state commitment.
///
/// Accounts:
/// 0: `[signer, writable]` state authority and settlement account
///                          (operator for active/operator-correct; challenger
///                          for challenger-correct)
/// 1: `[writable]`         PendingCommitment PDA
/// 2: `[writable]`         delegated account
/// 3: `[writable]`         DelegationRecord PDA
/// 4: `[writable]`         DelegationMetadata PDA
/// 5: `[]`                 selected finalized StateBuffer PDA
/// 6: `[]`                 DelegationCluster PDA
/// 7: `[]`                 system program, required by system CPI
/// 8: `[]`                 terminal Challenge PDA for challenger state, optional
#[inline(never)]
pub fn process_finalize_commitment(
    accounts: &[AccountView],
    data: &[u8],
) -> ProgramResult {
    let (
        [
            state_authority, // force multi-line
            pending_commitment,
            delegated_account,
            delegation_record,
            delegation_metadata,
            state_buffer,
            delegation_cluster,
            _system_program,
        ],
        optional_accounts,
    ) = require_n_accounts_with_optionals!(accounts, 8);

    let challenge = match optional_accounts.len() {
        0 => None,
        1 => Some(&optional_accounts[0]),
        _ => return Err(ProgramError::InvalidInstructionData),
    };

    require_eq!(data.len(), 0, ProgramError::InvalidInstructionData);
    require_signer!(state_authority);
    if !state_authority.is_writable() || !delegated_account.is_writable() {
        return Err(ProgramError::Immutable);
    }

    require_owned_pda(
        delegated_account,
        &crate::fast::ID,
        "delegated account",
    )?;
    require_owned_pda(
        pending_commitment,
        &crate::fast::ID,
        "pending commitment",
    )?;
    if let Some(challenge) = challenge {
        require_owned_pda(challenge, &crate::fast::ID, "challenge")?;
    }

    let pending_data = pending_commitment.try_borrow()?;
    let pending = PendingCommitment::decode(pending_data.as_ref())?;
    validate_pending_commitment(
        &pending,
        pending_commitment,
        delegated_account,
    )?;
    let challenge_data = challenge.map(AccountView::try_borrow).transpose()?;
    let challenge_state = challenge_data
        .as_ref()
        .map(|data| Challenge::decode(data.as_ref()))
        .transpose()?;

    // CHECKPOINT: A resolver decision makes the selected state immediately
    // finalizable, so resolved paths bypass the happy-path window and approval
    // checks. Until a settlement vault exists, the selected state's authority
    // also signs and settles its lamport delta.
    let selected_state =
        match (pending.status(), pending.resolved_state_source()) {
            (PENDING_COMMITMENT_STATUS_ACTIVE, None) => {
                // Finalize operator state after approval and the challenge window.
                require_eq!(
                    challenge.is_none(),
                    true,
                    ProgramError::InvalidInstructionData
                );
                require_eq_keys!(
                    pending.operator_identity(),
                    state_authority.address(),
                    DlpError::InvalidAuthority
                );
                validate_happy_path(&pending)?;
                SelectedState::from_pending(&pending)
            }
            (
                PENDING_COMMITMENT_STATUS_RESOLVED_OPERATOR,
                Some(RESOLVED_STATE_SOURCE_OPERATOR_COMMITMENT),
            ) => {
                // Finalize operator state selected by the dispute resolver.
                require_eq!(
                    challenge.is_none(),
                    true,
                    ProgramError::InvalidInstructionData
                );
                require_eq_keys!(
                    pending.operator_identity(),
                    state_authority.address(),
                    DlpError::InvalidAuthority
                );
                SelectedState::from_pending(&pending)
            }
            (
                PENDING_COMMITMENT_STATUS_RESOLVED_CHALLENGER,
                Some(RESOLVED_STATE_SOURCE_CHALLENGER_REVEAL),
            ) => {
                // Finalize challenger state selected by the dispute resolver.
                let challenge =
                    challenge.ok_or(ProgramError::NotEnoughAccountKeys)?;
                let challenge_state = challenge_state
                    .as_ref()
                    .ok_or(ProgramError::InvalidAccountData)?;
                validate_challenge(
                    challenge_state,
                    challenge,
                    pending_commitment,
                    state_authority,
                    state_buffer,
                    &pending,
                )?;
                SelectedState::from_challenge(challenge_state)
            }
            _ => return Err(ProgramError::InvalidInstructionData),
        };

    let delegation_cluster_data = delegation_cluster.try_borrow()?;
    let delegation_cluster_state =
        DelegationCluster::decode(delegation_cluster_data.as_ref())?;
    validate_delegation_cluster(
        &delegation_cluster_state,
        delegation_cluster,
        &pending,
    )?;

    require_initialized_delegation_record(
        delegated_account,
        delegation_record,
        true,
    )?;
    require_eq_keys!(
        pending.delegation_record(),
        delegation_record.address(),
        ProgramError::InvalidAccountData
    );
    require_initialized_delegation_metadata(
        delegated_account,
        delegation_metadata,
        true,
    )?;

    let commit_id_bytes = pending.commit_id().to_le_bytes();
    require_initialized_pda(
        state_buffer,
        &[
            STATE_BUFFER_SEED,
            delegated_account.address().as_ref(),
            &commit_id_bytes,
            state_authority.address().as_ref(),
        ],
        &crate::fast::ID,
        false,
        "state buffer",
    )?;

    let record_lamports = validate_delegation_record(
        delegation_record,
        delegation_cluster,
        selected_state.owner,
    )?;
    validate_delegation_metadata(delegation_metadata, pending.commit_id())?;

    let state_buffer_data = state_buffer.try_borrow()?;
    let raw_state = validate_state_buffer(
        state_buffer_data.as_ref(),
        state_authority,
        delegated_account,
        pending.commit_id(),
        selected_state.data_hash,
    )?;

    delegated_account.resize(raw_state.len())?;
    settle_lamports(
        state_authority,
        delegated_account,
        record_lamports,
        selected_state.lamports,
    )?;
    require_rent_exempt(delegated_account)?;

    delegated_account
        .try_borrow_mut()?
        .as_mut()
        .copy_from_slice(raw_state);
    drop(state_buffer_data);

    {
        let mut delegation_record_data = delegation_record.try_borrow_mut()?;
        let delegation_record_state =
            DelegationRecord::try_from_bytes_with_discriminator_mut(
                &mut delegation_record_data,
            )
            .map_err(to_pinocchio_program_error)?;
        delegation_record_state.lamports = selected_state.lamports;
    }

    {
        let mut metadata =
            DelegationMetadataFast::from_account(delegation_metadata)?;
        metadata.set_last_commit_id(pending.commit_id());
    }

    drop(delegation_cluster_data);
    drop(pending_data);
    *PendingCommitment::decode_mut(pending_commitment)?.status_mut()? =
        PENDING_COMMITMENT_STATUS_FINALIZED;

    Ok(())
}

fn validate_pending_commitment(
    pending: &PendingCommitmentView<'_>,
    pending_commitment: &AccountView,
    delegated_account: &AccountView,
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
    require_eq_keys!(
        pending.account_pubkey(),
        delegated_account.address(),
        DlpError::InvalidDelegatedAccount
    );
    require_eq!(
        pending.active_challenge().is_none(),
        true,
        ProgramError::InvalidInstructionData
    );

    Ok(())
}

fn validate_happy_path(pending: &PendingCommitmentView<'_>) -> ProgramResult {
    require_ge!(
        pending.approval_count(),
        pending.approval_threshold(),
        ProgramError::InvalidInstructionData
    );
    require_eq!(
        pending.approval_threshold(),
        1,
        ProgramError::InvalidAccountData
    );
    require_eq!(
        pending.selected_verifiers().len(),
        1,
        ProgramError::InvalidAccountData
    );
    require_eq!(
        pending
            .selected_verifiers()
            .get(0)
            .ok_or(ProgramError::InvalidAccountData)?
            .approved(),
        true,
        ProgramError::InvalidInstructionData
    );
    require_gt!(
        Clock::get()?.slot,
        pending.challenge_window_end_slot(),
        ProgramError::InvalidInstructionData
    );

    Ok(())
}

fn validate_delegation_cluster(
    delegation_cluster: &delegation_program_interface::v2::DelegationClusterView<'_>,
    delegation_cluster_account: &AccountView,
    pending: &PendingCommitmentView<'_>,
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

fn validate_delegation_record(
    delegation_record: &AccountView,
    delegation_cluster: &AccountView,
    expected_owner: &delegation_program_interface::compat::Pubkey,
) -> Result<u64, ProgramError> {
    let delegation_record_data = delegation_record.try_borrow()?;
    let delegation_record_state =
        DelegationRecord::try_from_bytes_with_discriminator(
            &delegation_record_data,
        )
        .map_err(to_pinocchio_program_error)?;

    require_eq_keys!(
        &delegation_record_state.authority,
        delegation_cluster.address(),
        DlpError::InvalidAuthority
    );
    require_eq_keys!(
        &delegation_record_state.owner,
        expected_owner,
        ProgramError::InvalidAccountData
    );

    Ok(delegation_record_state.lamports)
}

fn validate_delegation_metadata(
    delegation_metadata: &AccountView,
    commit_id: u64,
) -> ProgramResult {
    let metadata = DelegationMetadataFast::from_account(delegation_metadata)?;
    let expected_commit_id = metadata
        .last_commit_id()
        .checked_add(1)
        .ok_or(DlpError::Overflow)?;
    require_eq!(commit_id, expected_commit_id, DlpError::NonceOutOfOrder);

    match metadata.undelegation_requester()? {
        UndelegationRequester::None => Ok(()),
        UndelegationRequester::Validator => {
            Err(DlpError::AlreadyUndelegated.into())
        }
        UndelegationRequester::OwnerProgram => {
            Err(DlpError::OwnerRequestedUndelegation.into())
        }
    }
}

fn validate_state_buffer<'a>(
    data: &'a [u8],
    state_authority: &AccountView,
    delegated_account: &AccountView,
    commit_id: u64,
    expected_data_hash: &[u8; 32],
) -> Result<&'a [u8], ProgramError> {
    let state = StateBuffer::decode(data)?;

    if state.discriminator() != StateBuffer::DISCRIMINATOR {
        return Err(ProgramError::InvalidAccountData);
    }
    require_eq_keys!(
        state.authority(),
        state_authority.address(),
        DlpError::InvalidAuthority
    );
    require_eq_keys!(
        state.account_pubkey(),
        delegated_account.address(),
        DlpError::InvalidDelegatedAccount
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

    Ok(state.payload().as_slice())
}

fn settle_lamports(
    state_authority: &AccountView,
    delegated_account: &AccountView,
    record_lamports: u64,
    committed_lamports: u64,
) -> ProgramResult {
    require_ge!(
        delegated_account.lamports(),
        record_lamports,
        DlpError::InvalidDelegatedState
    );

    match committed_lamports.cmp(&record_lamports) {
        std::cmp::Ordering::Greater => {
            system::Transfer {
                from: state_authority,
                to: delegated_account,
                lamports: committed_lamports
                    .checked_sub(record_lamports)
                    .ok_or(DlpError::Overflow)?,
            }
            .invoke()?;
        }
        std::cmp::Ordering::Less => {
            let delta = record_lamports
                .checked_sub(committed_lamports)
                .ok_or(DlpError::Overflow)?;
            delegated_account.lamports_decrement_by(delta)?;
            state_authority.lamports_increment_by(delta)?;
        }
        std::cmp::Ordering::Equal => {}
    }

    Ok(())
}

struct SelectedState<'a> {
    lamports: u64,
    owner: &'a delegation_program_interface::compat::Pubkey,
    data_hash: &'a [u8; 32],
}

impl<'a> SelectedState<'a> {
    fn from_pending(pending: &'a PendingCommitmentView<'_>) -> Self {
        Self {
            lamports: pending.lamports(),
            owner: pending.owner(),
            data_hash: pending.data_hash(),
        }
    }

    fn from_challenge(challenge: &'a ChallengeView<'_>) -> Self {
        Self {
            lamports: challenge.challenger_lamports(),
            owner: challenge.challenger_owner(),
            data_hash: challenge.challenger_data_hash(),
        }
    }
}

fn validate_challenge(
    challenge: &ChallengeView<'_>,
    challenge_account: &AccountView,
    pending_commitment: &AccountView,
    state_authority: &AccountView,
    state_buffer: &AccountView,
    pending: &PendingCommitmentView<'_>,
) -> ProgramResult {
    if challenge.discriminator() != Challenge::DISCRIMINATOR {
        return Err(ProgramError::InvalidAccountData);
    }

    let commit_id_bytes = pending.commit_id().to_le_bytes();
    require_initialized_pda(
        challenge_account,
        &[
            CHALLENGE_SEED,
            pending.account_pubkey().as_ref(),
            &commit_id_bytes,
            challenge.challenger_identity().as_ref(),
        ],
        &crate::fast::ID,
        false,
        "challenge",
    )?;
    require_eq!(
        challenge.status(),
        CHALLENGE_STATUS_TERMINAL,
        ProgramError::InvalidInstructionData
    );
    require_eq!(
        challenge.outcome(),
        CHALLENGE_OUTCOME_CHALLENGER_CORRECT_OPERATOR_SLASHED,
        ProgramError::InvalidInstructionData
    );
    require_eq_keys!(
        challenge.pending_commitment(),
        pending_commitment.address(),
        ProgramError::InvalidAccountData
    );
    require_eq!(
        challenge.state_commitment_hash(),
        pending.state_commitment_hash(),
        ProgramError::InvalidAccountData
    );
    require_eq_keys!(
        challenge.challenger_identity(),
        state_authority.address(),
        DlpError::InvalidAuthority
    );
    require_eq_keys!(
        challenge.challenger_state_buffer(),
        state_buffer.address(),
        ProgramError::InvalidAccountData
    );

    Ok(())
}

fn require_rent_exempt(account: &AccountView) -> ProgramResult {
    require_ge!(
        account.lamports(),
        Rent::get()?.try_minimum_balance(account.data_len())?,
        DlpError::InsufficientRent
    );

    Ok(())
}
