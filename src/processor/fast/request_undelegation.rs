use dlp_api::consts::DEFAULT_UNDELEGATION_REQUEST_TIMEOUT_SLOTS;
use pinocchio::{
    address::Address,
    cpi::Signer,
    error::ProgramError,
    instruction::seeds,
    sysvars::{clock::Clock, Sysvar},
    AccountView, ProgramResult,
};

use super::to_pinocchio_program_error;
use crate::{
    error::DlpError,
    pda,
    processor::{fast::utils::pda::create_pda, utils::curve::is_on_curve_fast},
    require, require_eq, require_eq_keys, require_n_accounts,
    requires::{
        is_uninitialized_account, require_initialized_delegation_metadata,
        require_initialized_delegation_record, require_owned_pda, require_pda,
        require_signer, require_uninitialized_pda, UndelegationRequestCtx,
    },
    state::{
        DelegationMetadataFast, DelegationRecord, UndelegationRequest,
        UndelegationRequester,
    },
};

/// Request undelegation for one delegated account.
///
/// Accounts:
///
/// 0: `[signer, writable]` payer
/// 1: `[signer]`           delegated account
/// 2: `[]`                 owner program of the delegated account
/// 3: `[writable]`         undelegation request PDA
/// 4: `[]`                 delegation record PDA
/// 5: `[writable]`         delegation metadata PDA
/// 6: `[]`                 system program
pub fn process_request_undelegation(
    _program_id: &Address,
    accounts: &[AccountView],
    data: &[u8],
) -> ProgramResult {
    require!(data.is_empty(), ProgramError::InvalidInstructionData);

    let [
        payer, // force multi-line
        delegated_account,
        owner_program,
        undelegation_request_account,
        delegation_record_account,
        delegation_metadata_account,
        _system_program,
    ] = require_n_accounts!(accounts, 7);

    require_signer(payer, "payer")?;
    require!(payer.is_writable(), ProgramError::Immutable);
    require_signer(delegated_account, "delegated account")?;
    require_owned_pda(
        delegated_account,
        &crate::fast::ID,
        "delegated account",
    )?;

    require!(
        !is_on_curve_fast(delegated_account.address()),
        DlpError::RequestUndelegationOnCurveAccount
    );

    require_initialized_delegation_record(
        delegated_account,
        delegation_record_account,
        false,
    )?;
    require_initialized_delegation_metadata(
        delegated_account,
        delegation_metadata_account,
        false,
    )?;

    let delegation_record_data = delegation_record_account.try_borrow()?;
    let delegation_record =
        DelegationRecord::try_from_bytes_with_discriminator(
            &delegation_record_data,
        )
        .map_err(to_pinocchio_program_error)?;

    require_eq_keys!(
        &delegation_record.owner,
        owner_program.address(),
        ProgramError::InvalidAccountOwner
    );

    let mut delegation_metadata =
        DelegationMetadataFast::from_account(delegation_metadata_account)?;
    delegation_metadata
        .set_undelegation_requester(UndelegationRequester::OwnerProgram);
    let last_commit_id_at_request = delegation_metadata.last_commit_id();

    drop(delegation_record_data);
    drop(delegation_metadata);

    let request_seeds = &[
        pda::UNDELEGATION_REQUEST_TAG,
        delegated_account.address().as_ref(),
    ];

    if is_uninitialized_account(undelegation_request_account) {
        let created_slot = Clock::get()?.slot;
        let expires_at_slot = created_slot
            .checked_add(DEFAULT_UNDELEGATION_REQUEST_TIMEOUT_SLOTS)
            .ok_or(DlpError::Overflow)?;

        let request_bump = require_uninitialized_pda(
            undelegation_request_account,
            request_seeds,
            &crate::fast::ID,
            true,
            UndelegationRequestCtx,
        )?;

        create_pda(
            undelegation_request_account,
            &crate::fast::ID,
            UndelegationRequest::size_with_discriminator(),
            &[Signer::from(&seeds!(
                pda::UNDELEGATION_REQUEST_TAG,
                delegated_account.address().as_ref(),
                &[request_bump]
            ))],
            payer,
        )?;

        let request = UndelegationRequest {
            delegated_account: *delegated_account.address(),
            owner_program: *owner_program.address(),
            rent_payer: *payer.address(),
            created_slot,
            expires_at_slot,
            last_commit_id_at_request,
            bump: request_bump,
            _padding: [0; 7],
        };
        let mut request_data = undelegation_request_account.try_borrow_mut()?;
        request
            .to_bytes_with_discriminator(&mut request_data)
            .map_err(to_pinocchio_program_error)?;

        return Ok(());
    }

    let request_bump = require_pda(
        undelegation_request_account,
        request_seeds,
        &crate::fast::ID,
        true,
        "undelegation request",
    )?;
    require_owned_pda(
        undelegation_request_account,
        &crate::fast::ID,
        "undelegation request",
    )?;

    let mut request_data = undelegation_request_account.try_borrow_mut()?;
    let request = UndelegationRequest::try_from_bytes_with_discriminator_mut(
        &mut request_data,
    )
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
    require_eq!(
        request.bump,
        request_bump,
        DlpError::InvalidUndelegationRequest
    );

    // Refresh only the base-chain commit checkpoint. Repeated requests must not
    // reset or extend the timeout, but they are allowed to make rollback an
    // explicit owner-program decision after finalized base state has changed.
    request.last_commit_id_at_request = last_commit_id_at_request;

    Ok(())
}
