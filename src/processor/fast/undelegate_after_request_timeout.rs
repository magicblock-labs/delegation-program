use pinocchio::{
    address::Address,
    cpi::Signer,
    error::ProgramError,
    instruction::seeds,
    sysvars::{clock::Clock, Sysvar},
    AccountView, ProgramResult,
};

use super::{process_undelegation_with_cpi, to_pinocchio_program_error};
use crate::{
    error::DlpError,
    pda,
    processor::fast::utils::pda::{close_pda, create_pda},
    require, require_eq, require_eq_keys, require_ge, require_n_accounts,
    requires::{
        is_uninitialized_account, require_initialized_commit_record,
        require_initialized_commit_state,
        require_initialized_delegation_metadata,
        require_initialized_delegation_record, require_initialized_pda,
        require_owned_pda, require_pda, require_signer,
        require_uninitialized_pda, UndelegateBufferCtx,
    },
    state::{
        CommitRecord, DelegationMetadata, DelegationRecord, UndelegationRequest,
    },
};

/// Permissionless timeout path for a requested undelegation.
///
/// This intentionally returns the currently available base-chain delegated
/// account state. It does not accept or apply any pending validator commit.
///
/// Data-loss warning:
/// this is a rollback/escape hatch for validator non-response. If the
/// ephemeral validator has newer state that was never finalized on the base
/// chain, that state is intentionally not used here and can be lost from the
/// returned account's perspective. The safety property is that this path never
/// trusts fresh validator data after timeout; the tradeoff is that Program A
/// gets back only the last base-chain state available in the delegated account.
///
/// Accounts:
///
///  0: `[signer, writable]` caller
///  1: `[writable]`         delegated account
///  2: `[]`                 owner program of the delegated account
///  3: `[writable]`         undelegate buffer PDA
///  4: `[writable]`         undelegation request PDA
///  5: `[writable]`         delegation record PDA
///  6: `[writable]`         delegation metadata PDA
///  7: `[writable]`         request rent payer
///  8: `[writable]`         delegation rent payer
///  9: `[writable]`         commit state PDA
/// 10: `[writable]`         commit record PDA
/// 11: `[writable]`         commit reimbursement account
/// 12: `[]`                 system program
pub fn process_undelegate_after_request_timeout(
    _program_id: &Address,
    accounts: &[AccountView],
    _data: &[u8],
) -> ProgramResult {
    let [caller, delegated_account, owner_program, undelegate_buffer_account, undelegation_request_account, delegation_record_account, delegation_metadata_account, request_rent_payer, delegation_rent_payer, commit_state_account, commit_record_account, commit_reimbursement, system_program] =
        require_n_accounts!(accounts, 13);

    require_signer(caller, "caller")?;
    require!(caller.is_writable(), ProgramError::Immutable);
    require_owned_pda(
        delegated_account,
        &crate::fast::ID,
        "delegated account",
    )?;
    require!(delegated_account.is_writable(), ProgramError::Immutable);

    let request = load_valid_request(
        delegated_account,
        owner_program,
        undelegation_request_account,
        request_rent_payer,
    )?;
    let current_slot = Clock::get()?.slot;
    require_ge!(
        current_slot,
        request.expires_at_slot,
        DlpError::UndelegationRequestNotExpired
    );

    require_initialized_delegation_record(
        delegated_account,
        delegation_record_account,
        true,
    )?;
    require_initialized_delegation_metadata(
        delegated_account,
        delegation_metadata_account,
        true,
    )?;

    let (delegation_owner, delegation_metadata) = {
        let delegation_record_data = delegation_record_account.try_borrow()?;
        let delegation_record =
            DelegationRecord::try_from_bytes_with_discriminator(
                &delegation_record_data,
            )
            .map_err(to_pinocchio_program_error)?;

        let delegation_metadata_data =
            delegation_metadata_account.try_borrow()?;
        let delegation_metadata =
            DelegationMetadata::try_from_bytes_with_discriminator(
                &delegation_metadata_data,
            )
            .map_err(to_pinocchio_program_error)?;

        (delegation_record.owner, delegation_metadata)
    };

    require_eq_keys!(
        &delegation_owner,
        owner_program.address(),
        ProgramError::InvalidAccountOwner
    );
    // CHECKPOINT: A timeout request is bound to the delegation nonce observed
    // when it was created. If undelegation is still desired after the nonce
    // changes, one possible design is to let request_undelegation refresh an
    // existing request so it records the current nonce.
    require_eq!(
        request.delegation_nonce_at_request,
        delegation_metadata.last_update_nonce,
        DlpError::InvalidUndelegationRequest
    );
    require_eq_keys!(
        &delegation_metadata.rent_payer,
        delegation_rent_payer.address(),
        DlpError::InvalidReimbursementAddressForDelegationRent
    );
    require!(delegation_rent_payer.is_writable(), ProgramError::Immutable);

    if delegated_account.is_data_empty() {
        unsafe {
            delegated_account.assign(owner_program.address());
        }
    } else {
        undelegate_with_buffer_cpi(
            caller,
            delegated_account,
            owner_program,
            undelegate_buffer_account,
            delegation_metadata,
            system_program,
        )?;
    }

    // If a validator started a commit but did not finish finalizing it before
    // timeout, the commit PDAs are cleanup-only inputs. Do not move their data
    // into the delegated account. That would turn this rollback path into a
    // late validator-state acceptance path.
    cleanup_pending_commit(
        delegated_account,
        commit_state_account,
        commit_record_account,
        commit_reimbursement,
    )?;

    close_pda(undelegation_request_account, request_rent_payer)?;
    close_pda(delegation_record_account, delegation_rent_payer)?;
    close_pda(delegation_metadata_account, delegation_rent_payer)?;

    Ok(())
}

fn load_valid_request(
    delegated_account: &AccountView,
    owner_program: &AccountView,
    undelegation_request_account: &AccountView,
    request_rent_payer: &AccountView,
) -> Result<UndelegationRequest, ProgramError> {
    let request_bump = require_initialized_pda(
        undelegation_request_account,
        &[
            pda::UNDELEGATION_REQUEST_TAG,
            delegated_account.address().as_ref(),
        ],
        &crate::fast::ID,
        true,
        "undelegation request",
    )?;

    require!(request_rent_payer.is_writable(), ProgramError::Immutable);

    let request_data = undelegation_request_account.try_borrow()?;
    let request =
        *UndelegationRequest::try_from_bytes_with_discriminator(&request_data)
            .map_err(to_pinocchio_program_error)?;

    require_eq_keys!(
        &request.delegated_account,
        delegated_account.address(),
        DlpError::InvalidUndelegationRequest
    );
    require_eq_keys!(
        &request.owner_program,
        owner_program.address(),
        DlpError::InvalidUndelegationRequest
    );
    require_eq_keys!(
        &request.rent_payer,
        request_rent_payer.address(),
        DlpError::InvalidUndelegationRequest
    );
    require_eq!(
        request.bump,
        request_bump,
        DlpError::InvalidUndelegationRequest
    );

    Ok(request)
}

fn undelegate_with_buffer_cpi(
    caller: &AccountView,
    delegated_account: &AccountView,
    owner_program: &AccountView,
    undelegate_buffer_account: &AccountView,
    delegation_metadata: DelegationMetadata,
    system_program: &AccountView,
) -> ProgramResult {
    let undelegate_buffer_bump: u8 = require_uninitialized_pda(
        undelegate_buffer_account,
        &[
            pda::UNDELEGATE_BUFFER_TAG,
            delegated_account.address().as_ref(),
        ],
        &crate::fast::ID,
        true,
        UndelegateBufferCtx,
    )?;

    create_pda(
        undelegate_buffer_account,
        &crate::fast::ID,
        delegated_account.data_len(),
        &[Signer::from(&seeds!(
            pda::UNDELEGATE_BUFFER_TAG,
            delegated_account.address().as_ref(),
            &[undelegate_buffer_bump]
        ))],
        caller,
    )?;

    (*undelegate_buffer_account.try_borrow_mut()?)
        .copy_from_slice(&delegated_account.try_borrow()?);

    process_undelegation_with_cpi(
        caller,
        delegated_account,
        owner_program,
        undelegate_buffer_account,
        &[Signer::from(&seeds!(
            pda::UNDELEGATE_BUFFER_TAG,
            delegated_account.address().as_ref(),
            &[undelegate_buffer_bump]
        ))],
        delegation_metadata,
        system_program,
    )?;

    close_pda(undelegate_buffer_account, caller)
}

fn cleanup_pending_commit(
    delegated_account: &AccountView,
    commit_state_account: &AccountView,
    commit_record_account: &AccountView,
    commit_reimbursement: &AccountView,
) -> ProgramResult {
    require_pda(
        commit_state_account,
        &[pda::COMMIT_STATE_TAG, delegated_account.address().as_ref()],
        &crate::fast::ID,
        false,
        "commit state",
    )?;
    require_pda(
        commit_record_account,
        &[pda::COMMIT_RECORD_TAG, delegated_account.address().as_ref()],
        &crate::fast::ID,
        false,
        "commit record",
    )?;

    let commit_state_uninitialized =
        is_uninitialized_account(commit_state_account);
    let commit_record_uninitialized =
        is_uninitialized_account(commit_record_account);

    if commit_state_uninitialized && commit_record_uninitialized {
        return Ok(());
    }

    require_eq!(
        commit_state_uninitialized,
        commit_record_uninitialized,
        DlpError::InvalidPendingCommitState
    );

    require_initialized_commit_state(
        delegated_account,
        commit_state_account,
        true,
    )?;
    require_initialized_commit_record(
        delegated_account,
        commit_record_account,
        true,
    )?;

    {
        let commit_record_data = commit_record_account.try_borrow()?;
        let commit_record = CommitRecord::try_from_bytes_with_discriminator(
            &commit_record_data,
        )
        .map_err(to_pinocchio_program_error)?;

        require_eq_keys!(
            &commit_record.account,
            delegated_account.address(),
            DlpError::InvalidPendingCommitState
        );
        require_eq_keys!(
            &commit_record.identity,
            commit_reimbursement.address(),
            DlpError::InvalidPendingCommitState
        );
    }

    require!(commit_reimbursement.is_writable(), ProgramError::Immutable);

    // Request-timeout undelegation is a rollback/escape hatch. At this point the
    // validator has committed state into the commit PDAs, but that state has
    // not been finalized into the delegated account. Applying it here would let
    // this permissionless timeout path accept fresh validator state, which is
    // exactly what the timeout design forbids.
    //
    // We still close both commit PDAs so the delegated account cannot leave
    // orphaned DLP-owned accounts behind. The commit record identity is the
    // validator that created the pending commit, so sending both PDA balances to
    // that identity refunds commit rent/collateral without treating the pending
    // commit as valid state.
    close_pda(commit_state_account, commit_reimbursement)?;
    close_pda(commit_record_account, commit_reimbursement)
}
