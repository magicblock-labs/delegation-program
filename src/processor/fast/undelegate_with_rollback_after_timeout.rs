use pinocchio::{
    address::Address,
    error::ProgramError,
    sysvars::{clock::Clock, Sysvar},
    AccountView, ProgramResult,
};

use super::to_pinocchio_program_error;
use crate::{
    error::DlpError,
    pda,
    processor::fast::utils::pda::close_pda,
    require_eq, require_eq_keys, require_ge, require_n_accounts,
    requires::{
        is_uninitialized_account, require_initialized_commit_record,
        require_initialized_commit_state,
        require_initialized_delegation_metadata,
        require_initialized_delegation_record, require_initialized_pda,
        require_owned_pda, require_pda, require_signer,
    },
    state::{
        CommitRecord, DelegationMetadata, DelegationRecord, UndelegationRequest,
    },
};

/// Owner-program-authorized rollback after a requested undelegation times out.
///
/// This returns the currently available base-chain delegated
/// account state. It does not accept or apply any pending validator
/// commit, which means it might cause data-loss!
///
/// Data-loss Warning:
///
/// This is a rollback/escape hatch for validator non-response. If the ephemeral
/// validator has newer state that was never finalized on the base chain, that
/// state can be lost from the returned account's perspective. The tradeoff
/// is that the owner program gets the account back with only the last base-chain
/// state available in the delegated account.
///
/// Authorization:
///
/// The owner program authorizes rollback by invoking this instruction through
/// CPI and signing for the delegated account, same as the request/re-request
/// path.
///
/// This instruction releases the delegated account back to the owner program
/// without making an external undelegate CPI, because the owner program is
/// already on the invocation stack. The owner-program wrapper must preserve the
/// delegated account data before invoking DLP and restore it after DLP returns.
///
/// Accounts:
///
///  0: `[signer, writable]` delegated account
///  1: `[]`                 owner program of the delegated account
///  2: `[writable]`         undelegation request PDA
///  3: `[writable]`         delegation record PDA
///  4: `[writable]`         delegation metadata PDA
///  5: `[writable]`         delegation rent payer
///  6: `[writable]`         commit state PDA
///  7: `[writable]`         commit record PDA
///  8: `[writable]`         commit reimbursement account
pub fn process_undelegate_with_rollback_after_timeout(
    _program_id: &Address,
    accounts: &[AccountView],
    _data: &[u8],
) -> ProgramResult {
    let [delegated_account, owner_program, undelegation_request_account, delegation_record_account, delegation_metadata_account, delegation_rent_payer, commit_state_account, commit_record_account, commit_reimbursement] =
        require_n_accounts!(accounts, 9);

    require_signer(delegated_account, "delegated account")?;
    require_owned_pda(
        delegated_account,
        &crate::fast::ID,
        "delegated account",
    )?;

    let request =
        load_valid_request(delegated_account, undelegation_request_account)?;
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
    require_eq_keys!(
        &delegation_metadata.rent_payer,
        delegation_rent_payer.address(),
        DlpError::InvalidReimbursementAddressForDelegationRent
    );
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

    delegated_account.resize(0)?;
    unsafe {
        delegated_account.assign(owner_program.address());
    }

    close_pda(undelegation_request_account, delegation_rent_payer)?;
    close_pda(delegation_record_account, delegation_rent_payer)?;
    close_pda(delegation_metadata_account, delegation_rent_payer)?;

    Ok(())
}

fn load_valid_request(
    delegated_account: &AccountView,
    undelegation_request_account: &AccountView,
) -> Result<UndelegationRequest, ProgramError> {
    require_initialized_pda(
        undelegation_request_account,
        &[
            pda::UNDELEGATION_REQUEST_TAG,
            delegated_account.address().as_ref(),
        ],
        &crate::fast::ID,
        true,
        "undelegation request",
    )?;

    let request_data = undelegation_request_account.try_borrow()?;
    let request =
        *UndelegationRequest::try_from_bytes_with_discriminator(&request_data)
            .map_err(to_pinocchio_program_error)?;

    Ok(request)
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

    // Timeout rollback is an owner-program-authorized escape hatch. At this
    // point the validator has committed state into the commit PDAs, but that
    // state has not been finalized into the delegated account. Applying it here
    // would let this rollback path accept fresh validator state, which is
    // exactly what the timeout rollback design forbids.
    //
    // We still close both commit PDAs so the delegated account cannot leave
    // orphaned DLP-owned accounts behind. The commit record identity is the
    // validator that created the pending commit, so sending both PDA balances to
    // that identity refunds commit rent/collateral without treating the pending
    // commit as valid state.
    close_pda(commit_state_account, commit_reimbursement)?;
    close_pda(commit_record_account, commit_reimbursement)
}
