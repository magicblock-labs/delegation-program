use crate::consts::{
    BUFFER, COMMIT_RECORD, COMMIT_STATE, EXTERNAL_UNDELEGATE_DISCRIMINATOR, FEES_SESSION,
};
use crate::error::DlpError::{
    InvalidAccountDataAfterCPI, InvalidAuthority, InvalidDelegatedAccount,
    InvalidReimbursementAddressForDelegationRent, InvalidValidatorBalanceAfterCPI, Undelegatable,
};
use crate::processor::utils::curve::is_on_curve;
use crate::processor::utils::lamports::settle_lamports_balance;
use crate::processor::utils::loaders::{
    load_fees_vault, load_initialized_delegation_metadata, load_initialized_delegation_record,
    load_owned_pda, load_program, load_signer, load_uninitialized_pda, load_validator_fees_vault,
};
use crate::processor::utils::pda::{close_pda, close_pda_with_fees, create_pda};
use crate::processor::utils::verify::verify_state;
use crate::state::{CommitRecord, DelegationMetadata, DelegationRecord};
use borsh::BorshSerialize;
use solana_program::instruction::{AccountMeta, Instruction};
use solana_program::program::{invoke, invoke_signed};
use solana_program::program_error::ProgramError;
use solana_program::rent::Rent;
use solana_program::system_instruction::transfer;
use solana_program::sysvar::Sysvar;
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    system_program, {self},
};

/// Undelegate a delegated account
///
/// 1. If the new state is valid, copy the committed state to the buffer
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
    let [validator, delegated_account, owner_program, buffer, commit_state_account, commit_record_account, delegation_record_account, delegation_metadata_account, delegation_rent_reimbursement, fees_vault, validator_fees_vault, system_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Check accounts
    load_signer(validator)?;
    load_owned_pda(delegated_account, &crate::id())?;
    load_initialized_delegation_record(delegated_account, delegation_record_account)?;
    load_initialized_delegation_metadata(delegated_account, delegation_metadata_account)?;
    load_program(system_program, system_program::id())?;
    load_fees_vault(fees_vault)?;
    load_validator_fees_vault(validator, validator_fees_vault)?;

    // Check if there is a committed state
    let is_committed = is_state_committed(
        delegated_account,
        commit_state_account,
        commit_record_account,
    )?;

    // Load delegation record
    let delegation_record_data = delegation_record_account.try_borrow_data()?;
    let delegation_record =
        DelegationRecord::try_from_bytes_with_discriminator(&delegation_record_data)?;

    let commit_record_data = commit_record_account.try_borrow_data()?;
    let commit_record = if is_committed {
        let record = CommitRecord::try_from_bytes_with_discriminator(&commit_record_data)?;
        Some(record)
    } else {
        None
    };

    // Load delegated account metadata
    let delegation_metadata_data = delegation_metadata_account.try_borrow_data()?;
    let delegation_metadata =
        DelegationMetadata::try_from_bytes_with_discriminator(&delegation_metadata_data)?;

    // Check if the delegated account is undelegatable
    if !is_account_undelegatable(&delegation_metadata)? {
        return Err(Undelegatable.into());
    }

    // Check if the rent payer is correct
    if !delegation_metadata
        .rent_payer
        .eq(delegation_rent_reimbursement.key)
    {
        return Err(InvalidReimbursementAddressForDelegationRent.into());
    }

    let buffer_bump: u8 = load_uninitialized_pda(
        buffer,
        &[BUFFER, &delegated_account.key.to_bytes()],
        &crate::id(),
    )?;

    // Initialize the buffer PDA
    create_pda(
        buffer,
        &crate::id(),
        match is_committed {
            true => commit_state_account.data_len(),
            false => delegated_account.data_len(),
        },
        &[BUFFER, &delegated_account.key.to_bytes(), &[buffer_bump]],
        system_program,
        validator,
    )?;

    // Check passed owner and owner stored in the delegation record match
    if !delegation_record.owner.eq(owner_program.key) {
        return Err(ProgramError::InvalidAccountOwner);
    }

    // If there is a committed state, verify the state and compute the account lamports diff
    let committed_lamports_difference = if let Some(record) = commit_record.as_ref() {
        if !record.account.eq(delegated_account.key) {
            return Err(InvalidDelegatedAccount.into());
        }
        if !record.identity.eq(validator.key) {
            return Err(InvalidAuthority.into());
        }
        verify_state(validator, delegation_record, record, commit_state_account)?;
        delegation_record.lamports as i64 - record.lamports as i64
    } else {
        0
    };

    // Copy data in the buffer PDA
    (*buffer.try_borrow_mut_data()?).copy_from_slice(&match is_committed {
        true => commit_state_account.try_borrow_data()?,
        false => delegated_account.try_borrow_data()?,
    });

    // Dropping References
    drop(commit_record_data);
    drop(delegation_record_data);
    drop(delegation_metadata_data);

    if is_on_curve(delegated_account.key) || buffer.try_borrow_data()?.is_empty() {
        settle_lamports_balance(
            delegated_account,
            commit_state_account,
            committed_lamports_difference,
            validator_fees_vault,
        )?;
        delegated_account.assign(owner_program.key);
    } else {
        process_undelegation_with_cpi(
            validator,
            delegated_account,
            owner_program,
            buffer,
            commit_state_account,
            validator_fees_vault,
            system_program,
            delegation_metadata,
            buffer_bump,
            committed_lamports_difference,
        )?;
    }

    // Closing accounts
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
    close_pda(commit_record_account, validator)?;
    close_pda(commit_state_account, validator)?;
    close_pda(buffer, validator)?;
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
    buffer: &'a AccountInfo<'info>,
    commit_state_account: &'a AccountInfo<'info>,
    validator_fees_vault: &'a AccountInfo<'info>,
    system_program: &'a AccountInfo<'info>,
    metadata: DelegationMetadata,
    buffer_bump: u8,
    committed_lamports_difference: i64,
) -> ProgramResult {
    let delegated_account_balance_before_cpi = delegated_account.lamports();
    close_pda(delegated_account, validator)?;

    let validator_balance_before_cpi = validator.lamports();
    let signer_seeds: &[&[&[u8]]] = &[&[BUFFER, &delegated_account.key.to_bytes(), &[buffer_bump]]];
    cpi_external_undelegate(
        validator,
        delegated_account,
        buffer,
        system_program,
        owner_program.key,
        metadata,
        signer_seeds,
    )?;

    let min_rent = Rent::default().minimum_balance(delegated_account.data_len());
    if validator_balance_before_cpi
        != validator
            .lamports()
            .checked_add(min_rent)
            .ok_or(InvalidValidatorBalanceAfterCPI)?
    {
        return Err(InvalidValidatorBalanceAfterCPI.into());
    }

    if delegated_account.try_borrow_data()?.as_ref() != buffer.try_borrow_data()?.as_ref() {
        return Err(InvalidAccountDataAfterCPI.into());
    }

    let delegated_account_lamports_difference = delegated_account_balance_before_cpi
        .checked_sub(min_rent)
        .ok_or(InvalidDelegatedAccount)?;

    settle_lamports_balance_pda(
        validator,
        delegated_account,
        commit_state_account,
        validator_fees_vault,
        system_program,
        committed_lamports_difference,
        delegated_account_lamports_difference,
    )?;
    Ok(())
}

/// Settle lamports balance of the re-opened PDA
fn settle_lamports_balance_pda<'a, 'info>(
    validator: &'a AccountInfo<'info>,
    delegated_account: &'a AccountInfo<'info>,
    commit_state_account: &'a AccountInfo<'info>,
    validator_fees_vault: &'a AccountInfo<'info>,
    system_program: &'a AccountInfo<'info>,
    committed_lamports_difference: i64,
    delegated_account_lamports_difference: u64,
) -> Result<(), ProgramError> {
    let mut remaining_difference = delegated_account_lamports_difference;
    if committed_lamports_difference > 0 {
        remaining_difference = delegated_account_lamports_difference
            .checked_sub(committed_lamports_difference as u64)
            .ok_or(InvalidDelegatedAccount)?;
        invoke(
            &transfer(
                validator.key,
                validator_fees_vault.key,
                committed_lamports_difference as u64,
            ),
            &[
                validator.clone(),
                validator_fees_vault.clone(),
                system_program.clone(),
            ],
        )?;
    }

    if remaining_difference > 0 {
        invoke(
            &transfer(validator.key, delegated_account.key, remaining_difference),
            &[
                validator.clone(),
                delegated_account.clone(),
                system_program.clone(),
            ],
        )?;
    }

    if committed_lamports_difference < 0 {
        settle_lamports_balance(
            delegated_account,
            commit_state_account,
            committed_lamports_difference,
            validator_fees_vault,
        )?
    }
    Ok(())
}

/// Check if the account is undelegatable
fn is_account_undelegatable(metadata: &DelegationMetadata) -> Result<bool, ProgramError> {
    Ok(metadata.is_undelegatable
        || metadata.valid_until >= solana_program::clock::Clock::get()?.unix_timestamp)
}

/// Check if there is a committed state loading the committed state account and record PDAs
fn is_state_committed(
    delegated_account: &AccountInfo,
    commit_state_account: &AccountInfo,
    commit_record_account: &AccountInfo,
) -> Result<bool, ProgramError> {
    let is_committed = if commit_state_account.owner.eq(&system_program::id())
        && commit_record_account.owner.eq(&system_program::id())
    {
        load_uninitialized_pda(
            commit_state_account,
            &[COMMIT_STATE, &delegated_account.key.to_bytes()],
            &crate::id(),
        )?;
        load_uninitialized_pda(
            commit_record_account,
            &[COMMIT_RECORD, &delegated_account.key.to_bytes()],
            &crate::id(),
        )?;
        false
    } else {
        load_owned_pda(commit_state_account, &crate::id())?;
        load_owned_pda(commit_record_account, &crate::id())?;
        true
    };
    Ok(is_committed)
}

/// CPI to the original owner program to re-open the PDA with the new state
fn cpi_external_undelegate<'a, 'info>(
    payer: &'a AccountInfo<'info>,
    account_to_undelegate: &'a AccountInfo<'info>,
    buffer: &'a AccountInfo<'info>,
    system_program: &'a AccountInfo<'info>,
    program_id: &Pubkey,
    delegation_metadata: DelegationMetadata,
    signers_seeds: &[&[&[u8]]],
) -> ProgramResult {
    let mut data = EXTERNAL_UNDELEGATE_DISCRIMINATOR.to_vec();
    let serialized_seeds = delegation_metadata.seeds.try_to_vec()?;
    data.extend_from_slice(&serialized_seeds);

    let close_instruction = Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(*account_to_undelegate.key, false),
            AccountMeta::new(*buffer.key, true),
            AccountMeta::new(*payer.key, true),
            AccountMeta::new_readonly(*system_program.key, false),
        ],
        data,
    };

    invoke_signed(
        &close_instruction,
        &[account_to_undelegate.clone(), payer.clone(), buffer.clone()],
        signers_seeds,
    )
}
