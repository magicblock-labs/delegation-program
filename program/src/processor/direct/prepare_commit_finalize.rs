use delegation_program_interface as dpi;
use dpi::{
    error::DlpError, DelegationCluster, DelegationState,
    PrepareCommitFinalizeArgs, StateBuffer,
};
use pinocchio::{
    error::ProgramError,
    sysvars::{rent::Rent, Sysvar},
    AccountView, ProgramResult,
};
use pinocchio_system::instructions as system;
use wheels::{
    layout::Decodable, require, require_ge, require_le, require_n_accounts,
    require_owned_by, require_signer,
};

use crate::processor::internal::{
    validate_commit_state, validate_direct_cluster,
};

/// Grow a delegated account by one runtime-permitted step before finalization.
///
/// Repeat this instruction in the same transaction as finalization when the
/// target size exceeds one growth step.
///
/// Accounts:
/// 0: `[signer, writable]` current validator and rent payer
/// 1: `[writable]`         delegated account
/// 2: `[]`                 DelegationState PDA
/// 3: `[]`                 DelegationCluster PDA
/// 4: `[]`                 system program, required by system CPI
#[inline(never)]
pub fn process_prepare_commit_finalize(
    accounts: &[AccountView],
    data: &[u8],
) -> ProgramResult {
    let [
        current_validator, // force multi-line
        delegated_account,
        delegation_state,
        delegation_cluster,
        _system_program,
    ] = require_n_accounts!(accounts, 5);

    require_signer!(current_validator);
    require!(current_validator.is_writable(), ProgramError::Immutable);
    require_owned_by!(delegated_account, &crate::fast::ID);
    require!(delegated_account.is_writable(), ProgramError::Immutable);

    let args = PrepareCommitFinalizeArgs::decode(data)?;
    require_le!(
        args.target_data_len() as usize,
        StateBuffer::MAX_ACCOUNT_DATA_LEN,
        ProgramError::InvalidInstructionData
    );

    let cluster_data = delegation_cluster.try_borrow()?;
    let cluster = DelegationCluster::decode(cluster_data.as_ref())?;
    validate_direct_cluster(current_validator, delegation_cluster, &cluster)?;
    drop(cluster_data);

    let state_data = delegation_state.try_borrow()?;
    let state = DelegationState::decode(state_data.as_ref())?;
    validate_commit_state(
        delegated_account,
        delegation_state,
        delegation_cluster,
        &state,
        args.commit_seq(),
        false,
    )?;
    require_ge!(
        delegated_account.lamports(),
        state.lamports(),
        DlpError::InvalidDelegatedState
    );
    drop(state_data);

    let target_data_len = args.target_data_len() as usize;
    if delegated_account.data_len() >= target_data_len {
        return Ok(());
    }

    delegated_account.resize(
        delegated_account
            .data_len()
            .saturating_add(pinocchio::account::MAX_PERMITTED_DATA_INCREASE)
            .min(target_data_len),
    )?;

    let minimum_balance =
        Rent::get()?.try_minimum_balance(delegated_account.data_len())?;
    if delegated_account.lamports() < minimum_balance {
        system::Transfer {
            from: current_validator,
            to: delegated_account,
            lamports: minimum_balance
                .checked_sub(delegated_account.lamports())
                .ok_or(DlpError::Overflow)?,
        }
        .invoke()?;
    }

    Ok(())
}
