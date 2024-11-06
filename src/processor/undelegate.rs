use crate::consts::{
    BUFFER, COMMIT_RECORD, COMMIT_STATE, EXTERNAL_UNDELEGATE_DISCRIMINATOR, VALIDATOR_FEES_VAULT,
};
use crate::error::DlpError::{
    InvalidAccountDataAfterCPI, InvalidAuthority, InvalidDelegatedAccount,
    InvalidValidatorBalanceAfterCPI, Undelegatable,
};
use crate::pda::validator_fees_vault_pda_from_pubkey;
use crate::state::{CommitRecord, DelegationMetadata, DelegationRecord};
use crate::utils::balance_lamports::settle_lamports_balance;
use crate::utils::loaders::{
    load_initialized_pda, load_owned_pda, load_program, load_signer, load_uninitialized_pda,
};
use crate::utils::utils_account::AccountDeserialize;
use crate::utils::utils_pda::{close_pda, create_pda, ValidateEdwards};
use crate::utils::verify_state::verify_state;
use borsh::{BorshDeserialize, BorshSerialize};
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
    let [validator, delegated_account, owner_program, buffer, committed_state_account, committed_state_record, delegation_record, delegation_metadata, reimbursement, validator_fees_vault, system_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    load_signer(validator)?;
    load_owned_pda(delegated_account, &crate::id())?;
    load_owned_pda(delegation_record, &crate::id())?;
    load_owned_pda(delegation_metadata, &crate::id())?;
    load_program(system_program, system_program::id())?;

    // Check that the validator fees vault account is correct and initialized
    if !validator_fees_vault_pda_from_pubkey(validator.key).eq(validator_fees_vault.key) {
        return Err(InvalidAuthority.into());
    }
    load_initialized_pda(
        validator_fees_vault,
        &[VALIDATOR_FEES_VAULT, &validator.key.to_bytes()],
        &crate::id(),
        true,
    )?;

    // Check if the committed state is owned by the system program => no committed state
    let is_committed = if committed_state_account.owner.eq(&system_program::id())
        && committed_state_record.owner.eq(&system_program::id())
    {
        load_uninitialized_pda(
            committed_state_account,
            &[COMMIT_STATE, &delegated_account.key.to_bytes()],
            &crate::id(),
        )?;
        load_uninitialized_pda(
            committed_state_record,
            &[COMMIT_RECORD, &delegated_account.key.to_bytes()],
            &crate::id(),
        )?;
        false
    } else {
        load_owned_pda(committed_state_account, &crate::id())?;
        load_owned_pda(committed_state_record, &crate::id())?;
        true
    };

    // Load delegation record
    let delegation_data = delegation_record.try_borrow_data()?;
    let delegation = DelegationRecord::try_from_bytes(&delegation_data)?;

    let commit_record_data = committed_state_record.try_borrow_data()?;
    let commit_record = if is_committed {
        let record = CommitRecord::try_from_bytes(&commit_record_data)?;
        Some(record)
    } else {
        None
    };

    // Load delegated account metadata
    let metadata = DelegationMetadata::deserialize(&mut &**delegation_metadata.data.borrow())?;

    if !metadata.is_undelegatable
        && metadata.valid_until < solana_program::clock::Clock::get()?.unix_timestamp
    {
        return Err(Undelegatable.into());
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
            true => committed_state_account.data_len(),
            false => delegated_account.data_len(),
        },
        &[BUFFER, &delegated_account.key.to_bytes(), &[buffer_bump]],
        system_program,
        validator,
    )?;

    if !delegation.owner.eq(owner_program.key) {
        return Err(ProgramError::InvalidAccountOwner);
    }

    // If there is a committed state, verify the state
    let mut lamports_difference = 0;
    if let Some(record) = commit_record {
        if !record.account.eq(delegated_account.key) {
            return Err(InvalidDelegatedAccount.into());
        }
        verify_state(validator, delegation, record, committed_state_account)?;
        lamports_difference = metadata.last_update_lamports as i64 - record.lamports as i64;
    }

    let mut buffer_data = buffer.try_borrow_mut_data()?;
    let new_data = match is_committed {
        true => committed_state_account.try_borrow_data()?,
        false => delegated_account.try_borrow_data()?,
    };
    (*buffer_data).copy_from_slice(&new_data);

    // Dropping References
    drop(commit_record_data);
    drop(delegation_data);
    drop(buffer_data);
    drop(new_data);

    if delegated_account.is_on_curve() || buffer.try_borrow_data()?.is_empty() {
        delegated_account.assign(owner_program.key);

        // Settle lamports balance
        settle_lamports_balance(
            delegated_account,
            committed_state_account,
            lamports_difference,
            validator_fees_vault,
        )?;
    } else {
        // Closing delegated account before reopening it with the original owner
        let delegated_account_balance_before_cpi = delegated_account.lamports();
        close_pda(delegated_account, validator)?;

        // CPI to the owner program to re-open the PDA under the original owner
        let validator_balance_before_cpi = validator.lamports();
        let signer_seeds: &[&[&[u8]]] =
            &[&[BUFFER, &delegated_account.key.to_bytes(), &[buffer_bump]]];
        cpi_external_undelegate(
            validator,
            delegated_account,
            buffer,
            system_program,
            owner_program.key,
            metadata,
            signer_seeds,
        )?;

        // Asserts the validator was only charged for min rent to reopen the account
        let min_rent = Rent::default().minimum_balance(delegated_account.data_len());
        if validator_balance_before_cpi != validator.lamports() + min_rent {
            return Err(InvalidValidatorBalanceAfterCPI.into());
        }

        // Verify that delegated_account contains the expected data after CPI
        let delegated_data = delegated_account.try_borrow_data()?;
        let buffer_data = buffer.try_borrow_data()?;
        if delegated_data.as_ref() != buffer_data.as_ref() {
            return Err(InvalidAccountDataAfterCPI.into());
        }
        drop(buffer_data);
        drop(delegated_data);

        // Settle lamports: Transfer missing lamports from the validator to the delegated account
        let mut delegated_account_lamports_difference = delegated_account_balance_before_cpi
            .checked_sub(min_rent)
            .ok_or(InvalidDelegatedAccount)?;
        if lamports_difference > 0 {
            delegated_account_lamports_difference = delegated_account_lamports_difference
                .checked_sub(lamports_difference.unsigned_abs())
                .ok_or(InvalidDelegatedAccount)?;

            // Transfer lamports from the validator to the validator fees vault
            let transfer_instruction = transfer(
                validator.key,
                validator_fees_vault.key,
                lamports_difference.unsigned_abs(),
            );
            invoke(
                &transfer_instruction,
                &[
                    validator.clone(),
                    validator_fees_vault.clone(),
                    system_program.clone(),
                ],
            )?;
        }
        if delegated_account_lamports_difference > 0 {
            let transfer_instruction = transfer(
                validator.key,
                delegated_account.key,
                delegated_account_lamports_difference,
            );
            invoke(
                &transfer_instruction,
                &[
                    validator.clone(),
                    delegated_account.clone(),
                    system_program.clone(),
                ],
            )?;
        }

        // Settle lamports balance
        if lamports_difference < 0 {
            settle_lamports_balance(
                delegated_account,
                committed_state_account,
                lamports_difference,
                validator_fees_vault,
            )?;
        }
    }

    // Closing accounts
    close_pda(delegation_metadata, reimbursement)?;
    close_pda(committed_state_record, reimbursement)?;
    close_pda(delegation_record, reimbursement)?;
    close_pda(committed_state_account, reimbursement)?;
    close_pda(buffer, validator)?;
    Ok(())
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
