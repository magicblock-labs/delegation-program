use dlp_api::compat::borsh;
use pinocchio::{
    address::{address_eq, Address},
    cpi::{invoke_signed, Signer},
    error::ProgramError,
    instruction::{seeds, InstructionAccount, InstructionView},
    sysvars::{rent::Rent, Sysvar},
    AccountView, ProgramResult,
};
use pinocchio_log::log;
use pinocchio_system::instructions as system;

use super::to_pinocchio_program_error;
#[cfg(feature = "log-cost")]
use crate::compute;
use crate::{
    consts::{
        COMMIT_FEE_LAMPORTS, EXTERNAL_UNDELEGATE_DISCRIMINATOR,
        SESSION_FEE_LAMPORTS,
    },
    error::DlpError,
    pda,
    processor::fast::utils::pda::{close_pda, close_pda_with_fees, create_pda},
    requires::{
        require_initialized_delegation_metadata,
        require_initialized_delegation_record,
        require_initialized_protocol_fees_vault,
        require_initialized_validator_fees_vault, require_owned_pda,
        require_signer, require_uninitialized_pda, CommitRecordCtx,
        CommitStateAccountCtx, UndelegateBufferCtx,
    },
    state::{DelegationMetadata, DelegationRecord},
};

/// Undelegate a delegated account
///
/// Accounts:
///
///  0: `[signer]`   the validator account
///  1: `[writable]` the delegated account
///  2: `[]`         the owner program of the delegated account
///  3: `[writable]` the undelegate buffer PDA we use to store the data temporarily
///  4: `[]`         the commit state PDA
///  5: `[]`         the commit record PDA
///  6: `[writable]` the delegation record PDA
///  7: `[writable]` the delegation metadata PDA
///  8: `[]`         the rent reimbursement account
///  9: `[writable]` the protocol fees vault account
/// 10: `[writable]` the validator fees vault account
/// 11: `[]`         the system program (TODO (snawaz): soon to be removed from the requirement)
///
/// Requirements:
///
/// - delegated account is owned by delegation program
/// - delegation record is initialized
/// - delegation metadata is initialized
/// - protocol fees vault is initialized
/// - validator fees vault is initialized
/// - commit state is uninitialized
/// - commit record is uninitialized
/// - delegated account is NOT undelegatable
/// - owner program account matches the owner in the delegation record
/// - rent reimbursement account matches the rent payer in the delegation metadata
///
/// Steps:
///
/// - Close the delegation metadata
/// - Close the delegation record
/// - If delegated account has no data, assign to prev owner (and stop here)
/// - If there's data, create an "undelegate_buffer" and store the data in it
/// - Close the original delegated account
/// - CPI to the original owner to re-open the PDA with the original owner and the new state
/// - CPI will be signed by the undelegation buffer PDA and will call the external program
///   using the discriminator EXTERNAL_UNDELEGATE_DISCRIMINATOR
/// - Verify that the new state is the same as the committed state
/// - Close the undelegation buffer PDA
pub fn process_undelegate(
    _program_id: &Address,
    accounts: &[AccountView],
    _data: &[u8],
) -> ProgramResult {
    let [validator, delegated_account, owner_program, undelegate_buffer_account, commit_state_account, commit_record_account, delegation_record_account, delegation_metadata_account, rent_reimbursement, fees_vault, validator_fees_vault, system_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Check accounts
    require_signer(validator, "validator")?;
    require_owned_pda(
        delegated_account,
        &crate::fast::ID,
        "delegated account",
    )?;
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
    require_initialized_protocol_fees_vault(fees_vault, true)?;
    require_initialized_validator_fees_vault(
        validator,
        validator_fees_vault,
        true,
    )?;

    // Make sure there is no pending commits to be finalized before this call
    require_uninitialized_pda(
        commit_state_account,
        &[pda::COMMIT_STATE_TAG, delegated_account.address().as_ref()],
        &crate::fast::ID,
        false,
        CommitStateAccountCtx,
    )?;
    require_uninitialized_pda(
        commit_record_account,
        &[pda::COMMIT_RECORD_TAG, delegated_account.address().as_ref()],
        &crate::fast::ID,
        false,
        CommitRecordCtx,
    )?;

    // Load delegation record
    let delegation_record_data = delegation_record_account.try_borrow()?;
    let delegation_record =
        DelegationRecord::try_from_bytes_with_discriminator(
            &delegation_record_data,
        )
        .map_err(to_pinocchio_program_error)?;

    // Check passed owner and owner stored in the delegation record match
    if !address_eq(
        &delegation_record.owner.to_bytes().into(),
        owner_program.address(),
    ) {
        log!("Expected delegation record owner to be : ");
        Address::from(delegation_record.owner.to_bytes()).log();
        log!("but got : ");
        owner_program.address().log();
        return Err(ProgramError::InvalidAccountOwner);
    }

    // Load delegated account metadata
    let delegation_metadata_data = delegation_metadata_account.try_borrow()?;
    let delegation_metadata =
        DelegationMetadata::try_from_bytes_with_discriminator(
            &delegation_metadata_data,
        )
        .map_err(to_pinocchio_program_error)?;
    let delegation_last_update_nonce = delegation_metadata.last_update_nonce;

    // Check if the delegated account is undelegatable
    if !delegation_metadata.is_undelegatable {
        log!(
            "delegation metadata indicates the account is not undelegatable : "
        );
        delegation_metadata_account.address().log();
        return Err(DlpError::NotUndelegatable.into());
    }

    // Check if the rent payer is correct
    if !address_eq(
        &delegation_metadata.rent_payer.to_bytes().into(),
        rent_reimbursement.address(),
    ) {
        log!("Expected rent payer to be : ");
        Address::from(delegation_metadata.rent_payer.to_bytes()).log();
        log!("but got : ");
        rent_reimbursement.address().log();
        return Err(
            DlpError::InvalidReimbursementAddressForDelegationRent.into()
        );
    }

    // Dropping delegation references
    drop(delegation_record_data);
    drop(delegation_metadata_data);

    // If there is no data, we can just assign the owner back and we're done
    if delegated_account.is_data_empty() {
        // TODO - we could also do this fast-path if the data was non-empty but zeroed-out
        unsafe {
            delegated_account.assign(owner_program.address());
        }
        process_delegation_cleanup(
            delegation_record_account,
            delegation_metadata_account,
            rent_reimbursement,
            fees_vault,
            validator_fees_vault,
            delegation_last_update_nonce,
        )?;
        return Ok(());
    }

    // Initialize the undelegation buffer PDA
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
        validator,
    )?;

    // Copy data in the undelegation buffer PDA
    (*undelegate_buffer_account.try_borrow_mut()?)
        .copy_from_slice(&delegated_account.try_borrow()?);

    // Call a CPI to the owner program to give it back the new state
    process_undelegation_with_cpi(
        validator,
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

    // Done, close undelegation buffer
    close_pda(undelegate_buffer_account, validator)?;

    // Closing delegation accounts
    process_delegation_cleanup(
        delegation_record_account,
        delegation_metadata_account,
        rent_reimbursement,
        fees_vault,
        validator_fees_vault,
        delegation_last_update_nonce,
    )?;
    Ok(())
}

/// 1. Close the delegated account
/// 2. CPI to the owner program
/// 3. Check state
/// 4. Settle lamports balance
#[allow(clippy::too_many_arguments)]
pub(crate) fn process_undelegation_with_cpi(
    validator: &AccountView,
    delegated_account: &AccountView,
    owner_program: &AccountView,
    undelegate_buffer_account: &AccountView,
    undelegate_buffer_signer_seeds: &[Signer],
    delegation_metadata: DelegationMetadata,
    system_program: &AccountView,
) -> ProgramResult {
    let delegated_account_lamports_before_close = delegated_account.lamports();
    close_pda(delegated_account, validator)?;

    // Invoke the owner program's post-undelegation IX, to give the state back to the original program
    let validator_lamports_before_cpi = validator.lamports();

    cpi_external_undelegate(
        validator,
        delegated_account,
        undelegate_buffer_account,
        undelegate_buffer_signer_seeds,
        system_program,
        owner_program.address(),
        delegation_metadata,
    )?;

    let validator_lamports_after_cpi = validator.lamports();

    // Check that the validator lamports are exactly as expected
    let delegated_account_min_rent =
        Rent::get()?.try_minimum_balance(delegated_account.data_len())?;
    if validator_lamports_before_cpi
        != validator_lamports_after_cpi
            .checked_add(delegated_account_min_rent)
            .ok_or(DlpError::Overflow)?
    {
        return Err(DlpError::InvalidValidatorBalanceAfterCPI.into());
    }

    // Check that the owner program properly moved the state back into the original account during CPI
    if delegated_account.try_borrow()?.as_ref()
        != undelegate_buffer_account.try_borrow()?.as_ref()
    {
        return Err(DlpError::InvalidAccountDataAfterCPI.into());
    }

    // Return the extra lamports to the delegated account
    let delegated_account_extra_lamports =
        delegated_account_lamports_before_close
            .checked_sub(delegated_account_min_rent)
            .ok_or(DlpError::Overflow)?;

    system::Transfer {
        from: validator,
        to: delegated_account,
        lamports: delegated_account_extra_lamports,
    }
    .invoke()?;
    Ok(())
}

/// CPI to the original owner program to re-open the PDA with the new state
fn cpi_external_undelegate(
    payer: &AccountView,
    delegated_account: &AccountView,
    undelegate_buffer_account: &AccountView,
    undelegate_buffer_signer_seeds: &[Signer],
    system_program: &AccountView,
    owner_program_id: &Address,
    delegation_metadata: DelegationMetadata,
) -> ProgramResult {
    let data = {
        let mut data = Vec::with_capacity(32);
        data.extend_from_slice(&EXTERNAL_UNDELEGATE_DISCRIMINATOR);
        borsh::to_writer(&mut data, &delegation_metadata.seeds)
            .map_err(|_| ProgramError::BorshIoError)?;
        data
    };

    let external_undelegate_instruction = InstructionView {
        program_id: owner_program_id,
        data: &data,
        accounts: &[
            InstructionAccount::new(delegated_account.address(), true, false),
            InstructionAccount::new(
                undelegate_buffer_account.address(),
                true,
                true,
            ),
            InstructionAccount::new(payer.address(), true, true),
            InstructionAccount::new(system_program.address(), false, false),
        ],
    };

    invoke_signed(
        &external_undelegate_instruction,
        &[
            delegated_account,
            undelegate_buffer_account,
            payer,
            system_program,
        ],
        undelegate_buffer_signer_seeds,
    )
}

fn process_delegation_cleanup(
    delegation_record_account: &AccountView,
    delegation_metadata_account: &AccountView,
    rent_reimbursement: &AccountView,
    fees_vault: &AccountView,
    validator_fees_vault: &AccountView,
    delegation_last_update_nonce: u64,
) -> ProgramResult {
    let commit_count = delegation_last_update_nonce.saturating_sub(1);
    let commit_fee = COMMIT_FEE_LAMPORTS
        .checked_mul(commit_count)
        .ok_or(DlpError::Overflow)?;
    let total_fee_requested = commit_fee + SESSION_FEE_LAMPORTS;
    let total_lamports = delegation_record_account.lamports()
        + delegation_metadata_account.lamports();
    let mut fee_remaining = total_fee_requested.min(total_lamports);
    close_pda_with_fees(
        delegation_record_account,
        rent_reimbursement,
        fees_vault,
        validator_fees_vault,
        &mut fee_remaining,
    )?;
    close_pda_with_fees(
        delegation_metadata_account,
        rent_reimbursement,
        fees_vault,
        validator_fees_vault,
        &mut fee_remaining,
    )?;
    Ok(())
}
