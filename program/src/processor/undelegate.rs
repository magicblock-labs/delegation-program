use delegation_program_interface as dpi;
use dpi::{
    error::DlpError,
    pda::{
        DELEGATION_CLUSTER_SEED, DELEGATION_STATE_SEED, UNDELEGATE_BUFFER_SEED,
    },
    DelegationCluster, DelegationState, UndelegateArgs, UndelegationRequester,
};
use pinocchio::{
    address::address_eq, error::ProgramError, AccountView, ProgramResult,
};
use wheels::{
    layout::Decodable, require, require_eq, require_eq_keys, require_ge,
    require_n_accounts, require_owned_by, require_signer,
};

use crate::{
    legacy::processor::fast::utils::pda::close_pda,
    processor::internal::{owner_callback_data, return_account_to_owner},
    requires::{require_uninitialized_account, StandardCtx},
};

/// Return a delegated account to its original owner.
///
/// Accounts:
/// 0: `[signer, writable]` current cluster validator and buffer rent payer
/// 1: `[writable]`         delegated account
/// 2: `[]`                 original owner program
/// 3: `[writable]`         temporary undelegation buffer PDA
/// 4: `[writable]`         DelegationState PDA
/// 5: `[]`                 DelegationCluster PDA
/// 6: `[writable]`         DelegationState rent payer
/// 7: `[]`                 system program, forwarded to the owner callback
#[inline(never)]
pub fn process_undelegate(
    accounts: &[AccountView],
    data: &[u8],
) -> ProgramResult {
    let [current_validator, delegated_account, owner_program, undelegate_buffer, delegation_state, delegation_cluster, delegation_rent_payer, system_program] =
        require_n_accounts!(accounts, 8);

    require_signer!(current_validator);
    require!(current_validator.is_writable(), ProgramError::Immutable);
    require_owned_by!(delegated_account, &crate::fast::ID);
    require!(delegated_account.is_writable(), ProgramError::Immutable);
    require!(delegation_rent_payer.is_writable(), ProgramError::Immutable);
    require_eq_keys!(
        system_program.address(),
        &pinocchio_system::ID,
        ProgramError::IncorrectProgramId
    );

    let args = UndelegateArgs::decode(data)?;
    require!(
        delegated_account.data_len()
            <= pinocchio::account::MAX_PERMITTED_DATA_INCREASE,
        DlpError::AccountDataTooLargeForUndelegation
    );

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
        owner_program.address(),
        ProgramError::InvalidAccountOwner
    );
    require_eq_keys!(
        state.rent_payer(),
        delegation_rent_payer.address(),
        DlpError::InvalidReimbursementAddressForDelegationRent
    );
    require_ge!(
        delegated_account.lamports(),
        state.lamports(),
        DlpError::InvalidDelegatedState
    );
    match state.undelegation_requester() {
        value if value == UndelegationRequester::Validator.value() => {}
        value if value == UndelegationRequester::None.value() => {
            return Err(DlpError::NotUndelegatable.into());
        }
        value if value == UndelegationRequester::OwnerProgram.value() => {
            return Err(DlpError::OwnerRequestedUndelegation.into());
        }
        _ => return Err(ProgramError::InvalidAccountData),
    }
    let callback_data = owner_callback_data(&state)?;
    drop(state_data);

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
    require_eq_keys!(
        cluster.current_validator(),
        current_validator.address(),
        DlpError::InvalidAuthority
    );
    drop(cluster_data);

    let undelegate_buffer_bump = [args.undelegate_buffer_bump()];
    dpi::require_pda!(
        undelegate_buffer,
        &[
            UNDELEGATE_BUFFER_SEED,
            delegated_account.address().as_ref(),
            &undelegate_buffer_bump,
        ],
        &crate::fast::ID,
        true
    );
    require_uninitialized_account(
        undelegate_buffer,
        true,
        StandardCtx::new("undelegation buffer"),
    )?;
    if !delegated_account.is_data_empty() {
        require!(owner_program.executable(), ProgramError::InvalidAccountData);
    }

    return_account_to_owner(
        current_validator,
        delegated_account,
        owner_program,
        undelegate_buffer,
        system_program,
        args.undelegate_buffer_bump(),
        &callback_data,
    )?;
    close_pda(delegation_state, delegation_rent_payer)
}
