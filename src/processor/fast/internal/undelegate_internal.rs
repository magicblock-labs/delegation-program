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

use crate::{
    consts::{
        COMMIT_FEE_LAMPORTS, EXTERNAL_UNDELEGATE_DISCRIMINATOR,
        SESSION_FEE_LAMPORTS,
    },
    error::DlpError,
    pda,
    processor::fast::{
        to_pinocchio_program_error,
        utils::pda::{close_pda, close_pda_with_fees, create_pda},
    },
    require_eq_keys,
    requires::{
        require_initialized_delegation_metadata,
        require_initialized_delegation_record, require_initialized_pda,
        require_initialized_protocol_fees_vault,
        require_initialized_validator_fees_vault, require_owned_pda,
        require_uninitialized_pda, UndelegateBufferCtx,
    },
    state::{
        DelegationMetadata, DelegationRecord, UndelegationRequest,
        UndelegationRequester,
    },
};

pub(crate) struct AutoUndelegationAccounts<'a> {
    pub(crate) owner_program: &'a AccountView,
    pub(crate) undelegate_buffer_account: &'a AccountView,
    pub(crate) rent_reimbursement: &'a AccountView,
    pub(crate) fees_vault: &'a AccountView,
    pub(crate) undelegation_request_account: &'a AccountView,
}

pub(crate) fn parse_auto_undelegation_accounts(
    accounts: &[AccountView],
) -> Result<Option<AutoUndelegationAccounts<'_>>, ProgramError> {
    match accounts.len() {
        0 => Ok(None),
        5 => {
            let [owner_program, undelegate_buffer_account, rent_reimbursement, fees_vault, undelegation_request_account] =
                TryInto::<&[_; 5]>::try_into(accounts)
                    .map_err(|_| DlpError::InfallibleError)?;
            Ok(Some(AutoUndelegationAccounts {
                owner_program,
                undelegate_buffer_account,
                rent_reimbursement,
                fees_vault,
                undelegation_request_account,
            }))
        }
        _ => Err(ProgramError::InvalidInstructionData),
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn process_auto_undelegation_if_requested(
    requester: UndelegationRequester,
    validator: &AccountView,
    delegated_account: &AccountView,
    delegation_record_account: &AccountView,
    delegation_metadata_account: &AccountView,
    validator_fees_vault: &AccountView,
    system_program: &AccountView,
    auto_accounts: Option<AutoUndelegationAccounts<'_>>,
) -> ProgramResult {
    if requester == UndelegationRequester::None {
        return Ok(());
    }

    let Some(auto_accounts) = auto_accounts else {
        if requester == UndelegationRequester::Validator {
            log!(
                "WARN: validator-requested undelegation skipped; \
                 auto-undelegation accounts were not provided"
            );
            return Ok(());
        }
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    process_undelegation(UndelegationAccounts {
        validator,
        delegated_account,
        owner_program: auto_accounts.owner_program,
        undelegate_buffer_account: auto_accounts.undelegate_buffer_account,
        delegation_record_account,
        delegation_metadata_account,
        rent_reimbursement: auto_accounts.rent_reimbursement,
        fees_vault: auto_accounts.fees_vault,
        validator_fees_vault,
        system_program,
        // For OwnerProgram requests, see UndelegationRequest::rent_payer
        // for the rent-payer invariant. Validator requests do not use the
        // request PDA.
        request_accounts: if requester == UndelegationRequester::OwnerProgram {
            Some((
                auto_accounts.undelegation_request_account,
                auto_accounts.rent_reimbursement,
            ))
        } else {
            None
        },
    })
}

pub(crate) struct UndelegationAccounts<'a> {
    pub(crate) validator: &'a AccountView,
    pub(crate) delegated_account: &'a AccountView,
    pub(crate) owner_program: &'a AccountView,
    pub(crate) undelegate_buffer_account: &'a AccountView,
    pub(crate) delegation_record_account: &'a AccountView,
    pub(crate) delegation_metadata_account: &'a AccountView,
    pub(crate) rent_reimbursement: &'a AccountView,
    pub(crate) fees_vault: &'a AccountView,
    pub(crate) validator_fees_vault: &'a AccountView,
    pub(crate) system_program: &'a AccountView,
    pub(crate) request_accounts: Option<(&'a AccountView, &'a AccountView)>,
}

pub(crate) fn process_undelegation(
    accounts: UndelegationAccounts<'_>,
) -> ProgramResult {
    if let Some((undelegation_request_account, request_rent_payer)) =
        accounts.request_accounts
    {
        require_valid_undelegation_request(
            accounts.delegated_account,
            accounts.owner_program,
            undelegation_request_account,
            request_rent_payer,
        )?;
    };

    require_owned_pda(
        accounts.delegated_account,
        &crate::fast::ID,
        "delegated account",
    )?;
    require_initialized_delegation_record(
        accounts.delegated_account,
        accounts.delegation_record_account,
        true,
    )?;
    require_initialized_delegation_metadata(
        accounts.delegated_account,
        accounts.delegation_metadata_account,
        true,
    )?;
    require_initialized_protocol_fees_vault(accounts.fees_vault, true)?;
    require_initialized_validator_fees_vault(
        accounts.validator,
        accounts.validator_fees_vault,
        true,
    )?;

    // Load delegation record
    let delegation_record_data =
        accounts.delegation_record_account.try_borrow()?;
    let delegation_record =
        DelegationRecord::try_from_bytes_with_discriminator(
            &delegation_record_data,
        )
        .map_err(to_pinocchio_program_error)?;

    // Check passed owner and owner stored in the delegation record match
    if !address_eq(
        &delegation_record.owner.to_bytes().into(),
        accounts.owner_program.address(),
    ) {
        log!("Expected delegation record owner to be : ");
        Address::from(delegation_record.owner.to_bytes()).log();
        log!("but got : ");
        accounts.owner_program.address().log();
        return Err(ProgramError::InvalidAccountOwner);
    }

    // Load delegated account metadata
    let delegation_metadata_data =
        accounts.delegation_metadata_account.try_borrow()?;
    let delegation_metadata =
        DelegationMetadata::try_from_bytes_with_discriminator(
            &delegation_metadata_data,
        )
        .map_err(to_pinocchio_program_error)?;
    let delegation_last_commit_id = delegation_metadata.last_commit_id;

    // Check if undelegation has been requested for the delegated account.
    if delegation_metadata.undelegation_requester == UndelegationRequester::None
    {
        log!("delegation metadata has no undelegation requester: ");
        accounts.delegation_metadata_account.address().log();
        return Err(DlpError::NotUndelegatable.into());
    }

    // Check if the rent payer is correct
    if !address_eq(
        &delegation_metadata.rent_payer.to_bytes().into(),
        accounts.rent_reimbursement.address(),
    ) {
        log!("Expected rent payer to be : ");
        Address::from(delegation_metadata.rent_payer.to_bytes()).log();
        log!("but got : ");
        accounts.rent_reimbursement.address().log();
        return Err(
            DlpError::InvalidReimbursementAddressForDelegationRent.into()
        );
    }
    if let Some((_undelegation_request_account, request_rent_payer)) =
        accounts.request_accounts
    {
        require_eq_keys!(
            request_rent_payer.address(),
            accounts.rent_reimbursement.address(),
            DlpError::InvalidUndelegationRequest
        );
    }

    // Dropping delegation references
    drop(delegation_record_data);
    drop(delegation_metadata_data);

    // If there is no data, we can just assign the owner back and we're done
    if accounts.delegated_account.is_data_empty() {
        // TODO - we could also do this fast-path if the data was non-empty but zeroed-out
        unsafe {
            accounts
                .delegated_account
                .assign(accounts.owner_program.address());
        }
        process_delegation_cleanup(
            accounts.delegation_record_account,
            accounts.delegation_metadata_account,
            accounts.rent_reimbursement,
            accounts.fees_vault,
            accounts.validator_fees_vault,
            delegation_last_commit_id,
        )?;
        if let Some((undelegation_request_account, request_rent_payer)) =
            accounts.request_accounts
        {
            close_pda(undelegation_request_account, request_rent_payer)?;
        }
        return Ok(());
    }

    // Initialize the undelegation buffer PDA
    let undelegate_buffer_bump: u8 = require_uninitialized_pda(
        accounts.undelegate_buffer_account,
        &[
            pda::UNDELEGATE_BUFFER_TAG,
            accounts.delegated_account.address().as_ref(),
        ],
        &crate::fast::ID,
        true,
        UndelegateBufferCtx,
    )?;

    create_pda(
        accounts.undelegate_buffer_account,
        &crate::fast::ID,
        accounts.delegated_account.data_len(),
        &[Signer::from(&seeds!(
            pda::UNDELEGATE_BUFFER_TAG,
            accounts.delegated_account.address().as_ref(),
            &[undelegate_buffer_bump]
        ))],
        accounts.validator,
    )?;

    // Copy data in the undelegation buffer PDA
    (*accounts.undelegate_buffer_account.try_borrow_mut()?)
        .copy_from_slice(&accounts.delegated_account.try_borrow()?);

    // Call a CPI to the owner program to give it back the new state
    process_undelegation_with_cpi(
        accounts.validator,
        accounts.delegated_account,
        accounts.owner_program,
        accounts.undelegate_buffer_account,
        &[Signer::from(&seeds!(
            pda::UNDELEGATE_BUFFER_TAG,
            accounts.delegated_account.address().as_ref(),
            &[undelegate_buffer_bump]
        ))],
        delegation_metadata,
        accounts.system_program,
    )?;

    // Done, close undelegation buffer
    close_pda(accounts.undelegate_buffer_account, accounts.validator)?;

    // Closing delegation accounts
    process_delegation_cleanup(
        accounts.delegation_record_account,
        accounts.delegation_metadata_account,
        accounts.rent_reimbursement,
        accounts.fees_vault,
        accounts.validator_fees_vault,
        delegation_last_commit_id,
    )?;
    if let Some((undelegation_request_account, request_rent_payer)) =
        accounts.request_accounts
    {
        close_pda(undelegation_request_account, request_rent_payer)?;
    }
    Ok(())
}

fn require_valid_undelegation_request(
    delegated_account: &AccountView,
    owner_program: &AccountView,
    undelegation_request_account: &AccountView,
    request_rent_payer: &AccountView,
) -> ProgramResult {
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
        UndelegationRequest::try_from_bytes_with_discriminator(&request_data)
            .map_err(to_pinocchio_program_error)?;

    if !address_eq(
        &request.delegated_account.to_bytes().into(),
        delegated_account.address(),
    ) || !address_eq(
        &request.owner_program.to_bytes().into(),
        owner_program.address(),
    ) || !address_eq(
        &request.rent_payer.to_bytes().into(),
        request_rent_payer.address(),
    ) {
        return Err(DlpError::InvalidUndelegationRequest.into());
    }

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
    delegation_last_commit_id: u64,
) -> ProgramResult {
    let commit_count = delegation_last_commit_id.saturating_sub(1);
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
