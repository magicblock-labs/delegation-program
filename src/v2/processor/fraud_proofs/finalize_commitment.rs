use dlp_api::{
    error::DlpError,
    state::{DelegationMetadataFast, DelegationRecord, UndelegationRequester},
    v2::{
        pda::{
            DELEGATION_CLUSTER_SEED, PENDING_COMMITMENT_SEED, STATE_BUFFER_SEED,
        },
        DelegationCluster, PendingCommitment, PendingCommitmentView,
        StateBuffer, PENDING_COMMITMENT_STATUS_ACTIVE,
        PENDING_COMMITMENT_STATUS_FINALIZED,
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
    require_gt, require_n_accounts, require_signer,
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
/// 0: `[signer, writable]` operator identity and lamport settlement account
/// 1: `[writable]`         PendingCommitment PDA
/// 2: `[writable]`         delegated account
/// 3: `[writable]`         DelegationRecord PDA
/// 4: `[writable]`         DelegationMetadata PDA
/// 5: `[]`                 finalized StateBuffer PDA
/// 6: `[]`                 DelegationCluster PDA
/// 7: `[]`                 system program, required by system CPI
#[inline(never)]
pub fn process_finalize_commitment(
    accounts: &[AccountView],
    data: &[u8],
) -> ProgramResult {
    let [
        operator, // force multi-line
        pending_commitment,
        delegated_account,
        delegation_record,
        delegation_metadata,
        state_buffer,
        delegation_cluster,
        _system_program,
    ] = require_n_accounts!(accounts, 8);

    require_eq!(data.len(), 0, ProgramError::InvalidInstructionData);
    require_signer!(operator);
    if !operator.is_writable() || !delegated_account.is_writable() {
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

    let pending_data = pending_commitment.try_borrow()?;
    let pending = PendingCommitment::decode(pending_data.as_ref())?;
    validate_pending_commitment(
        &pending,
        pending_commitment,
        operator,
        delegated_account,
    )?;

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
            operator.address().as_ref(),
        ],
        &crate::fast::ID,
        false,
        "state buffer",
    )?;

    let record_lamports = validate_delegation_record(
        delegation_record,
        delegation_cluster,
        &pending,
    )?;
    validate_delegation_metadata(delegation_metadata, pending.commit_id())?;

    let state_buffer_data = state_buffer.try_borrow()?;
    let raw_state = validate_state_buffer(
        state_buffer_data.as_ref(),
        operator,
        delegated_account,
        &pending,
    )?;

    delegated_account.resize(raw_state.len())?;
    settle_lamports(
        operator,
        delegated_account,
        record_lamports,
        pending.lamports(),
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
        delegation_record_state.lamports = pending.lamports();
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
    operator: &AccountView,
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
        pending.operator_identity(),
        operator.address(),
        DlpError::InvalidAuthority
    );
    require_eq_keys!(
        pending.account_pubkey(),
        delegated_account.address(),
        DlpError::InvalidDelegatedAccount
    );
    require_eq!(
        pending.status(),
        PENDING_COMMITMENT_STATUS_ACTIVE,
        ProgramError::InvalidInstructionData
    );
    require_eq!(
        pending.active_challenge().is_none(),
        true,
        ProgramError::InvalidInstructionData
    );
    require_eq!(
        pending.resolved_state_source().is_none(),
        true,
        ProgramError::InvalidInstructionData
    );
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
    delegation_cluster: &dlp_api::v2::DelegationClusterView<'_>,
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
    pending: &PendingCommitmentView<'_>,
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
        pending.owner(),
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
    operator: &AccountView,
    delegated_account: &AccountView,
    pending: &PendingCommitmentView<'_>,
) -> Result<&'a [u8], ProgramError> {
    let state = StateBuffer::decode(data)?;

    if state.discriminator() != StateBuffer::DISCRIMINATOR {
        return Err(ProgramError::InvalidAccountData);
    }
    require_eq_keys!(
        state.authority(),
        operator.address(),
        DlpError::InvalidAuthority
    );
    require_eq_keys!(
        state.account_pubkey(),
        delegated_account.address(),
        DlpError::InvalidDelegatedAccount
    );
    require_eq!(
        state.commit_id(),
        pending.commit_id(),
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
        pending.data_hash(),
        ProgramError::InvalidInstructionData
    );

    Ok(state.payload().as_slice())
}

fn settle_lamports(
    operator: &AccountView,
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
                from: operator,
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
            operator.lamports_increment_by(delta)?;
        }
        std::cmp::Ordering::Equal => {}
    }

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
