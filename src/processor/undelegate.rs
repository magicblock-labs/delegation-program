use crate::consts::{EXTERNAL_UNDELEGATE_DISCRIMINATOR, FEES_SESSION};
use crate::error::DlpError::{
    InvalidAccountDataAfterCPI, InvalidDelegatedAccount,
    InvalidReimbursementAddressForDelegationRent, InvalidValidatorBalanceAfterCPI, Undelegatable,
};
use crate::processor::utils::curve::is_on_curve;
use crate::processor::utils::loaders::{
    load_initialized_delegation_metadata, load_initialized_delegation_record,
    load_initialized_fees_vault, load_initialized_validator_fees_vault, load_owned_pda,
    load_program, load_signer, load_uninitialized_pda,
};
use crate::processor::utils::pda::{close_pda, close_pda_with_fees, create_pda};
use crate::state::{DelegationMetadata, DelegationRecord};
use crate::undelegation_buffer_seeds_from_delegated_account;
use borsh::BorshSerialize;
use solana_program::instruction::{AccountMeta, Instruction};
use solana_program::program::{invoke, invoke_signed};
use solana_program::program_error::ProgramError;
use solana_program::rent::Rent;
use solana_program::system_instruction::transfer;
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    system_program, {self},
};

/// Undelegate a delegated account
///
/// 2. Close the locked account
/// 3a. If on curve account or no data, close and reopen with prev owner
/// 3b. CPI to the original owner to re-open the PDA with the original owner and the new state
/// - The CPI will be signed by the buffer PDA and will call the external program
///   using the discriminator EXTERNAL_UNDELEGATE_DISCRIMINATOR
/// 4. Verify that the new state is the same as the committed state
/// 5. Close the buffer PDA
/// 6. Settle the lamports balance
/// 7. Close the state diff account (if exists)
/// 8. Close the commit state record (if exists)
/// 9. Close the delegation record
///
///
/// Accounts expected: Authority Record, Buffer PDA, Delegated PDA
pub fn process_undelegate(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    _data: &[u8],
) -> ProgramResult {
    let [validator, delegated_account, owner_program, undelegation_buffer_account, delegation_record_account, delegation_metadata_account, delegation_rent_reimbursement, fees_vault, validator_fees_vault, system_program] =
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
        return Err(Undelegatable.into());
    }

    // Check if the rent payer is correct
    if !delegation_metadata
        .rent_payer
        .eq(delegation_rent_reimbursement.key)
    {
        return Err(InvalidReimbursementAddressForDelegationRent.into());
    }

    // Initialize the undelegation buffer PDA
    let undelegation_buffer_bump: u8 = load_uninitialized_pda(
        undelegation_buffer_account,
        undelegation_buffer_seeds_from_delegated_account!(delegated_account.key),
        &crate::id(),
    )?;
    create_pda(
        undelegation_buffer_account,
        &crate::id(),
        delegated_account.data_len(),
        undelegation_buffer_seeds_from_delegated_account!(delegated_account.key),
        undelegation_buffer_bump,
        system_program,
        validator,
    )?;

    // Copy data in the undelegation buffer PDA
    (*undelegation_buffer_account.try_borrow_mut_data()?)
        .copy_from_slice(&delegated_account.try_borrow_data()?);

    // Dropping delegation references
    drop(delegation_record_data);
    drop(delegation_metadata_data);

    // Closing delegation accounts
    close_pda_with_fees(
        delegation_record_account,
        delegation_rent_reimbursement,
        &[fees_vault, validator_fees_vault],
        FEES_SESSION,
    )?;
    close_pda_with_fees(
        delegation_metadata_account,
        delegation_rent_reimbursement,
        &[fees_vault, validator_fees_vault],
        FEES_SESSION,
    )?;

    // If there is an owner program, give it back ownership to the delegated account
    if is_on_curve(delegated_account.key) {
        delegated_account.assign(owner_program.key);
    } else {
        process_undelegation_with_cpi(
            validator,
            delegated_account,
            owner_program,
            undelegation_buffer_account,
            undelegation_buffer_bump,
            delegation_metadata,
            system_program,
        )?;
    }

    // Done, close undelegation buffer
    close_pda(undelegation_buffer_account, validator)?;

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
    undelegation_buffer_account: &'a AccountInfo<'info>,
    undelegation_buffer_bump: u8,
    delegation_metadata: DelegationMetadata,
    system_program: &'a AccountInfo<'info>,
) -> ProgramResult {
    let delegated_account_balance_before_cpi = delegated_account.lamports();

    close_pda(delegated_account, validator)?;

    let validator_balance_after_close_and_before_cpi = validator.lamports();

    cpi_external_undelegate(
        validator,
        delegated_account,
        undelegation_buffer_account,
        undelegation_buffer_bump,
        system_program,
        owner_program.key,
        delegation_metadata,
    )?;

    let delegated_account_min_rent = Rent::default().minimum_balance(delegated_account.data_len());
    if validator_balance_after_close_and_before_cpi
        != validator
            .lamports()
            .checked_add(delegated_account_min_rent)
            .ok_or(InvalidValidatorBalanceAfterCPI)?
    {
        return Err(InvalidValidatorBalanceAfterCPI.into());
    }

    let delegated_account_data_after_cpi = delegated_account.try_borrow_data()?;
    let undelegation_buffer_data_after_cpi = undelegation_buffer_account.try_borrow_data()?;
    if delegated_account_data_after_cpi.as_ref() != undelegation_buffer_data_after_cpi.as_ref() {
        return Err(InvalidAccountDataAfterCPI.into());
    }

    let delegated_account_extra_lamports = delegated_account_balance_before_cpi
        .checked_sub(delegated_account_min_rent)
        .ok_or(InvalidDelegatedAccount)?;

    if delegated_account_extra_lamports > 0 {
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
    }

    Ok(())
}

/// CPI to the original owner program to re-open the PDA with the new state
fn cpi_external_undelegate<'a, 'info>(
    payer: &'a AccountInfo<'info>,
    delegated_account: &'a AccountInfo<'info>,
    undelegation_buffer_account: &'a AccountInfo<'info>,
    undelegation_buffer_bump: u8,
    system_program: &'a AccountInfo<'info>,
    program_id: &Pubkey,
    delegation_metadata: DelegationMetadata,
) -> ProgramResult {
    // Generate the instruction to be called in the owner program
    let mut data = EXTERNAL_UNDELEGATE_DISCRIMINATOR.to_vec();
    let serialized_seeds = delegation_metadata.seeds.try_to_vec()?;
    data.extend_from_slice(&serialized_seeds);
    let external_undelegate_instruction = Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(*delegated_account.key, false),
            AccountMeta::new(*undelegation_buffer_account.key, true),
            AccountMeta::new(*payer.key, true),
            AccountMeta::new_readonly(*system_program.key, false),
        ],
        data,
    };

    // Generate the ephemeral balance PDA's signer seeds
    let undelegation_buffer_seeds: &[&[u8]] =
        undelegation_buffer_seeds_from_delegated_account!(delegated_account.key);
    let undelegation_buffer_bump_slice = &[undelegation_buffer_bump];
    let undelegation_buffer_signer_seeds =
        [undelegation_buffer_seeds, &[undelegation_buffer_bump_slice]].concat();

    // Invoke the owner program
    invoke_signed(
        &external_undelegate_instruction,
        &[
            delegated_account.clone(),
            payer.clone(),
            undelegation_buffer_account.clone(),
        ],
        &[&undelegation_buffer_signer_seeds],
    )
}
