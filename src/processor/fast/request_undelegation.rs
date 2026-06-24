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
    consts::DEFAULT_UNDELEGATION_REQUEST_TIMEOUT_SLOTS,
    error::DlpError,
    pda,
    processor::{fast::utils::pda::create_pda, utils::curve::is_on_curve_fast},
    require, require_eq_keys, require_ge, require_n_accounts,
    requires::{
        is_uninitialized_account, require_initialized_delegation_metadata,
        require_initialized_delegation_record, require_owned_pda, require_pda,
        require_signer, require_uninitialized_pda, UndelegationRequestCtx,
    },
    state::{DelegationMetadata, DelegationRecord, UndelegationRequest},
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
/// 5: `[]`                 delegation metadata PDA
/// 6: `[]`                 system program
pub fn process_request_undelegation(
    _program_id: &Address,
    accounts: &[AccountView],
    data: &[u8],
) -> ProgramResult {
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

    let delegation_metadata_data = delegation_metadata_account.try_borrow()?;
    let delegation_metadata =
        DelegationMetadata::try_from_bytes_with_discriminator(
            &delegation_metadata_data,
        )
        .map_err(to_pinocchio_program_error)?;

    drop(delegation_record_data);
    drop(delegation_metadata_data);

    let request_seeds = &[
        pda::UNDELEGATION_REQUEST_TAG,
        delegated_account.address().as_ref(),
    ];
    let timeout_slots = parse_timeout_slots(data)?;

    if is_uninitialized_account(undelegation_request_account) {
        let created_slot = Clock::get()?.slot;
        let expires_at_slot = created_slot
            .checked_add(timeout_slots)
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
            delegation_nonce_at_request: delegation_metadata.last_update_nonce,
            bump: request_bump,
            _padding: [0; 7],
        };
        let mut request_data = undelegation_request_account.try_borrow_mut()?;
        request
            .to_bytes_with_discriminator(&mut request_data)
            .map_err(to_pinocchio_program_error)?;

        return Ok(());
    }

    require_pda(
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

    let request_data = undelegation_request_account.try_borrow()?;
    let request =
        UndelegationRequest::try_from_bytes_with_discriminator(&request_data)
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

    Ok(())
}

fn parse_timeout_slots(data: &[u8]) -> Result<u64, ProgramError> {
    match data.len() {
        0 => Ok(DEFAULT_UNDELEGATION_REQUEST_TIMEOUT_SLOTS),
        8 => {
            let timeout_slots = u64::from_le_bytes(
                data.try_into()
                    .map_err(|_| ProgramError::InvalidInstructionData)?,
            );
            require_ge!(
                timeout_slots,
                DEFAULT_UNDELEGATION_REQUEST_TIMEOUT_SLOTS,
                DlpError::UndelegationRequestTimeoutTooShort
            );
            Ok(timeout_slots)
        }
        _ => Err(ProgramError::InvalidInstructionData),
    }
}
