use delegation_program_interface as dpi;
use dpi::{
    error::DlpError,
    pda::{DELEGATION_CLUSTER_SEED, DELEGATION_STATE_SEED},
    DelegationCluster, DelegationState, UndelegationRequester,
};
use pinocchio::{
    address::address_eq,
    error::ProgramError,
    sysvars::{rent::Rent, Sysvar},
    AccountView, ProgramResult,
};
use pinocchio_system::instructions as system;
use wheels::{
    layout::Decodable, require, require_eq, require_eq_keys, require_ge,
    require_owned_by, require_signer,
};

use crate::{
    apply_diff_in_place, legacy::processor::fast::utils::LamportsOperation,
};

pub(crate) enum CommitStateUpdate<'a> {
    FullData(&'a [u8]),
    Diff(dpi::diff::DiffSet<'a>),
}

impl CommitStateUpdate<'_> {
    fn data_len(&self) -> usize {
        match self {
            Self::FullData(data) => data.len(),
            Self::Diff(diff) => diff.changed_len(),
        }
    }
}

pub(crate) struct CommitFinalizeInput<'a> {
    pub(crate) commit_id: u64,
    pub(crate) lamports: u64,
    pub(crate) owner: &'a dpi::compat::Pubkey,
    pub(crate) allow_undelegation: bool,
    pub(crate) state_update: CommitStateUpdate<'a>,
}

pub(crate) fn apply_commit_finalize(
    current_validator: &AccountView,
    delegated_account: &AccountView,
    delegation_state: &AccountView,
    delegation_cluster: &AccountView,
    input: CommitFinalizeInput<'_>,
) -> ProgramResult {
    require_signer!(current_validator);
    require!(current_validator.is_writable(), ProgramError::Immutable);
    require_owned_by!(delegated_account, &crate::fast::ID);
    require!(delegated_account.is_writable(), ProgramError::Immutable);

    let cluster_data = delegation_cluster.try_borrow()?;
    let cluster = DelegationCluster::decode(cluster_data.as_ref())?;
    let cluster_bump = [cluster.bump()];
    dpi::require_initialized_pda!(
        delegation_cluster,
        &[
            DELEGATION_CLUSTER_SEED,
            cluster.cluster_id().as_ref(),
            &cluster_bump,
        ],
        &crate::fast::ID,
        false
    );
    require_eq!(
        &cluster.discriminator(),
        &DelegationCluster::DISCRIMINATOR,
        ProgramError::InvalidAccountData
    );
    require!(!cluster.paused(), ProgramError::InvalidAccountData);
    require_eq!(
        cluster.challenge_config().is_none(),
        true,
        ProgramError::InvalidInstructionData
    );
    require_eq_keys!(
        cluster.current_validator(),
        current_validator.address(),
        DlpError::InvalidAuthority
    );
    drop(cluster_data);

    let state_data = delegation_state.try_borrow()?;
    let state = DelegationState::decode(state_data.as_ref())?;
    let state_bump = [state.bump()];
    dpi::require_initialized_pda!(
        delegation_state,
        &[
            DELEGATION_STATE_SEED,
            delegated_account.address().as_ref(),
            &state_bump,
        ],
        &crate::fast::ID,
        true
    );
    require_eq!(
        &state.discriminator(),
        &DelegationState::DISCRIMINATOR,
        ProgramError::InvalidAccountData
    );
    require_eq_keys!(
        state.delegated_account(),
        delegated_account.address(),
        DlpError::InvalidDelegatedAccount
    );
    require_eq_keys!(
        state.delegation_cluster(),
        delegation_cluster.address(),
        ProgramError::InvalidAccountData
    );
    require_eq_keys!(
        state.original_owner(),
        input.owner,
        ProgramError::InvalidAccountData
    );
    require_eq!(
        input.commit_id,
        state
            .last_commit_id()
            .checked_add(1)
            .ok_or(DlpError::Overflow)?,
        DlpError::NonceOutOfOrder
    );

    let next_undelegation_requester = match state.undelegation_requester() {
        value if value == UndelegationRequester::None.value() => {
            if input.allow_undelegation {
                UndelegationRequester::Validator.value()
            } else {
                value
            }
        }
        value if value == UndelegationRequester::Validator.value() => {
            return Err(DlpError::AlreadyUndelegated.into());
        }
        value if value == UndelegationRequester::OwnerProgram.value() => {
            require!(
                input.allow_undelegation,
                DlpError::OwnerRequestedUndelegation
            );
            value
        }
        _ => return Err(ProgramError::InvalidAccountData),
    };
    let previous_lamports = state.lamports();
    drop(state_data);

    require_ge!(
        delegated_account.lamports(),
        previous_lamports,
        DlpError::InvalidDelegatedState
    );

    delegated_account.resize(input.state_update.data_len())?;
    settle_lamports(
        current_validator,
        delegated_account,
        previous_lamports,
        input.lamports,
    )?;
    require_ge!(
        delegated_account.lamports(),
        Rent::get()?.try_minimum_balance(delegated_account.data_len())?,
        DlpError::InsufficientRent
    );

    let mut account_data = delegated_account.try_borrow_mut()?;
    match input.state_update {
        CommitStateUpdate::FullData(data) => {
            account_data.as_mut().copy_from_slice(data);
        }
        CommitStateUpdate::Diff(diff) => {
            apply_diff_in_place(account_data.as_mut(), &diff)?;
        }
    }
    drop(account_data);

    let mut state = DelegationState::decode_mut(delegation_state)?;
    state.lamports_mut()?.set(input.lamports)?;
    state.last_commit_id_mut()?.set(input.commit_id)?;
    *state.undelegation_requester_mut()? = next_undelegation_requester;

    Ok(())
}

fn settle_lamports(
    current_validator: &AccountView,
    delegated_account: &AccountView,
    previous_lamports: u64,
    commit_lamports: u64,
) -> ProgramResult {
    match commit_lamports.cmp(&previous_lamports) {
        core::cmp::Ordering::Greater => system::Transfer {
            from: current_validator,
            to: delegated_account,
            lamports: commit_lamports
                .checked_sub(previous_lamports)
                .ok_or(DlpError::Overflow)?,
        }
        .invoke(),
        core::cmp::Ordering::Less => {
            let delta = previous_lamports
                .checked_sub(commit_lamports)
                .ok_or(DlpError::Overflow)?;
            delegated_account.lamports_decrement_by(delta)?;
            current_validator.lamports_increment_by(delta)
        }
        core::cmp::Ordering::Equal => Ok(()),
    }
}
