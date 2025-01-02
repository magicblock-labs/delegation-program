use crate::consts::{EXTERNAL_UNDELEGATE_DISCRIMINATOR, RENT_FEES_PERCENTAGE};
use crate::error::DlpError;
use crate::processor::utils::loaders::{
    load_initialized_delegation_metadata, load_initialized_delegation_record,
    load_initialized_fees_vault, load_initialized_validator_fees_vault, load_owned_pda,
    load_program, load_signer, load_uninitialized_pda,
};
use crate::processor::utils::pda::{close_pda, close_pda_with_fees, create_pda};
use crate::state::{DelegationMetadata, DelegationRecord};
use crate::{
    commit_record_seeds_from_delegated_account, commit_state_seeds_from_delegated_account,
    undelegate_buffer_seeds_from_delegated_account,
};
use borsh::BorshSerialize;
use solana_program::instruction::{AccountMeta, Instruction};
use solana_program::program::{invoke, invoke_signed};
use solana_program::program_error::ProgramError;
use solana_program::rent::Rent;
use solana_program::system_instruction::transfer;
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey, system_program,
};

/// Undelegate a delegated account
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
///
///
/// Accounts expected: Authority Record, Buffer PDA, Delegated PDA
pub fn process_undelegate(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    _data: &[u8],
) -> ProgramResult {
    let [validator, delegated_account, owner_program, undelegate_buffer_account, commit_state_account, commit_record_account, delegation_record_account, delegation_metadata_account, rent_reimbursement, fees_vault, validator_fees_vault, system_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Check accounts
    load_signer(validator)?;
    load_owned_pda(delegated_account, &crate::id())?;
    load_initialized_delegation_record(delegated_account, delegation_record_account, true)?;
    load_initialized_delegation_metadata(delegated_account, delegation_metadata_account, true)?;
    load_initialized_fees_vault(fees_vault, true)?;
    load_initialized_validator_fees_vault(validator, validator_fees_vault, true)?;
    load_program(system_program, system_program::id())?;

    // Make sure there is no pending commits to be finalized before this call
    load_uninitialized_pda(
        commit_state_account,
        commit_state_seeds_from_delegated_account!(delegated_account.key),
        &crate::id(),
        false,
    )?;
    load_uninitialized_pda(
        commit_record_account,
        commit_record_seeds_from_delegated_account!(delegated_account.key),
        &crate::id(),
        false,
    )?;

    // Load delegation record
    let delegation_record_data = delegation_record_account.try_borrow_data()?;
    let delegation_record =
        DelegationRecord::try_from_bytes_with_discriminator(&delegation_record_data)?;

    // Check passed owner and owner stored in the delegation record match
    if !delegation_record.owner.eq(owner_program.key) {
        return Err(ProgramError::InvalidAccountOwner);
    }

    // Load delegated account metadata
    let delegation_metadata_data = delegation_metadata_account.try_borrow_data()?;
    let delegation_metadata =
        DelegationMetadata::try_from_bytes_with_discriminator(&delegation_metadata_data)?;

    // Check if the delegated account is undelegatable
    if !delegation_metadata.is_undelegatable {
        return Err(DlpError::Undelegatable.into());
    }

    // Check if the rent payer is correct
    if !delegation_metadata.rent_payer.eq(rent_reimbursement.key) {
        return Err(DlpError::InvalidReimbursementAddressForDelegationRent.into());
    }

    // Dropping delegation references
    drop(delegation_record_data);
    drop(delegation_metadata_data);

    // If there is no program to call CPI to, we can just assign the owner back and we're done
    if delegated_account.data_is_empty() {
        // TODO - we could also do this fast-path if the data was non-empty but zeroed-out
        delegated_account.assign(owner_program.key);
        process_delegation_cleanup(
            delegation_record_account,
            delegation_metadata_account,
            rent_reimbursement,
            fees_vault,
            validator_fees_vault,
        )?;
        return Ok(());
    }

    // Initialize the undelegation buffer PDA
    let undelegate_buffer_seeds: &[&[u8]] =
        undelegate_buffer_seeds_from_delegated_account!(delegated_account.key);
    let undelegate_buffer_bump: u8 = load_uninitialized_pda(
        undelegate_buffer_account,
        undelegate_buffer_seeds,
        &crate::id(),
        true,
    )?;
    create_pda(
        undelegate_buffer_account,
        &crate::id(),
        delegated_account.data_len(),
        undelegate_buffer_seeds,
        undelegate_buffer_bump,
        system_program,
        validator,
    )?;

    // Copy data in the undelegation buffer PDA
    (*undelegate_buffer_account.try_borrow_mut_data()?)
        .copy_from_slice(&delegated_account.try_borrow_data()?);

    // Generate the ephemeral balance PDA's signer seeds
    let undelegate_buffer_bump_slice = &[undelegate_buffer_bump];
    let undelegate_buffer_signer_seeds =
        [undelegate_buffer_seeds, &[undelegate_buffer_bump_slice]].concat();

    // Call a CPI to the owner program to give it back the new state
    process_undelegation_with_cpi(
        validator,
        delegated_account,
        owner_program,
        undelegate_buffer_account,
        &undelegate_buffer_signer_seeds,
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
    )?;
    Ok(())
}

/// 1. Close the delegated account
/// 2. CPI to the owner program
/// 3. Check state
/// 4. Settle lamports balance
#[allow(clippy::too_many_arguments)]
fn process_undelegation_with_cpi<'a, 'info>(
    validator: &'a AccountInfo<'info>,
    delegated_account: &'a AccountInfo<'info>,
    owner_program: &'a AccountInfo<'info>,
    undelegate_buffer_account: &'a AccountInfo<'info>,
    undelegate_buffer_signer_seeds: &[&[u8]],
    delegation_metadata: DelegationMetadata,
    system_program: &'a AccountInfo<'info>,
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
        owner_program.key,
        delegation_metadata,
    )?;
    let validator_lamports_after_cpi = validator.lamports();

    // Check that the validator lamports are exactly as expected
    let delegated_account_min_rent = Rent::default().minimum_balance(delegated_account.data_len());
    if validator_lamports_before_cpi
        != validator_lamports_after_cpi
            .checked_add(delegated_account_min_rent)
            .ok_or(DlpError::Overflow)?
    {
        return Err(DlpError::InvalidValidatorBalanceAfterCPI.into());
    }

    // Check that the owner program properly moved the state back into the original account during CPI
    if delegated_account.try_borrow_data()?.as_ref()
        != undelegate_buffer_account.try_borrow_data()?.as_ref()
    {
        return Err(DlpError::InvalidAccountDataAfterCPI.into());
    }

    // Return the extra lamports to the delegated account
    let delegated_account_extra_lamports = delegated_account_lamports_before_close
        .checked_sub(delegated_account_min_rent)
        .ok_or(DlpError::Overflow)?;
    invoke(
        &transfer(
            validator.key,
            delegated_account.key,
            delegated_account_extra_lamports,
        ),
        &[
            validator.clone(),
            delegated_account.clone(),
            system_program.clone(),
        ],
    )?;

    Ok(())
}

/// CPI to the original owner program to re-open the PDA with the new state
fn cpi_external_undelegate<'a, 'info>(
    payer: &'a AccountInfo<'info>,
    delegated_account: &'a AccountInfo<'info>,
    undelegate_buffer_account: &'a AccountInfo<'info>,
    undelegate_buffer_signer_seeds: &[&[u8]],
    system_program: &'a AccountInfo<'info>,
    owner_program_id: &Pubkey,
    delegation_metadata: DelegationMetadata,
) -> ProgramResult {
    let mut data = EXTERNAL_UNDELEGATE_DISCRIMINATOR.to_vec();
    let serialized_seeds = delegation_metadata.seeds.try_to_vec()?;
    data.extend_from_slice(&serialized_seeds);
    let external_undelegate_instruction = Instruction {
        program_id: *owner_program_id,
        accounts: vec![
            AccountMeta::new(*delegated_account.key, false),
            AccountMeta::new(*undelegate_buffer_account.key, true),
            AccountMeta::new(*payer.key, true),
            AccountMeta::new_readonly(*system_program.key, false),
        ],
        data,
    };
    invoke_signed(
        &external_undelegate_instruction,
        &[
            delegated_account.clone(),
            undelegate_buffer_account.clone(),
            payer.clone(),
            system_program.clone(),
        ],
        &[undelegate_buffer_signer_seeds],
    )
}

fn process_delegation_cleanup<'a, 'info>(
    delegation_record_account: &'a AccountInfo<'info>,
    delegation_metadata_account: &'a AccountInfo<'info>,
    rent_reimbursement: &'a AccountInfo<'info>,
    fees_vault: &'a AccountInfo<'info>,
    validator_fees_vault: &'a AccountInfo<'info>,
) -> ProgramResult {
    // TODO - fix: there is a double-fee being applied because validator_claim_fees also applies protocol level fees
    // TODO - fix: we should probably only let the validator apply fees there? or not apply extra fees during validator_fees_claim
    close_pda_with_fees(
        delegation_record_account,
        rent_reimbursement,
        &[fees_vault, validator_fees_vault],
        RENT_FEES_PERCENTAGE,
    )?;
    close_pda_with_fees(
        delegation_metadata_account,
        rent_reimbursement,
        &[fees_vault, validator_fees_vault],
        RENT_FEES_PERCENTAGE,
    )?;
    Ok(())
}
