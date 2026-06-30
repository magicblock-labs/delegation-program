use pinocchio::{
    address::{address_eq, Address},
    error::ProgramError,
    AccountView, ProgramResult,
};

#[cfg(feature = "log-cost")]
use crate::compute;
use crate::{
    pda,
    processor::fast::internal::{process_undelegation, UndelegationAccounts},
    require_n_accounts, require_n_accounts_with_optionals,
    requires::{
        require_initialized_delegation_metadata,
        require_initialized_delegation_record,
        require_initialized_protocol_fees_vault,
        require_initialized_validator_fees_vault, require_owned_pda,
        require_signer, require_uninitialized_pda, CommitRecordCtx,
        CommitStateAccountCtx,
    },
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
/// - undelegation has been requested for the delegated account
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
    let (
        [
            validator, // force multi-line
            delegated_account,
            owner_program,
            undelegate_buffer_account,
            commit_state_account,
            commit_record_account,
            delegation_record_account,
            delegation_metadata_account,
            rent_reimbursement,
            fees_vault,
            validator_fees_vault,
            system_program,
        ],
        optional_accounts,
    ) = require_n_accounts_with_optionals!(accounts, 12);

    let request_accounts = match optional_accounts.len() {
        0 => None,
        2 => {
            let [
                undelegation_request_account, // force multi-line
                request_rent_payer,
            ] = require_n_accounts!(optional_accounts, 2);
            Some((undelegation_request_account, request_rent_payer))
        }
        _ => return Err(ProgramError::InvalidInstructionData),
    };

    let delegated_account_owner = unsafe { delegated_account.owner() };
    if !address_eq(delegated_account_owner, &crate::fast::ID)
        && address_eq(delegated_account_owner, owner_program.address())
    {
        return Ok(());
    }

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

    process_undelegation(UndelegationAccounts {
        validator,
        delegated_account,
        owner_program,
        undelegate_buffer_account,
        delegation_record_account,
        delegation_metadata_account,
        rent_reimbursement,
        fees_vault,
        validator_fees_vault,
        system_program,
        request_accounts,
    })
}
