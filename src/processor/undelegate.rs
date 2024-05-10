use solana_program::instruction::{AccountMeta, Instruction};
use solana_program::program::invoke_signed;
use solana_program::program_error::ProgramError;
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    pubkey::Pubkey,
    system_program, {self},
};

use crate::consts::BUFFER;
use crate::loaders::{load_initialized_pda, load_owned_pda, load_program, load_signer};
use crate::state::{CommitState, Delegation};
use crate::utils::{close_pda, AccountDeserialize};

const EXTERNAL_UNDELEGATE_DISCRIMINATOR: [u8; 8] = [175, 175, 109, 31, 13, 152, 155, 237];

/// Undelegate a delegated Pda
///
/// 1. If the state diff is valid, copy the committed state to the buffer PDA
/// 2. Close the locked account
/// 3. CPI to the original owner to re-open the PDA with the original owner and the new state
/// - The CPI will be signed by the buffer PDA and will call the external program
///   using the discriminator EXTERNAL_UNDELEGATE_DISCRIMINATOR
/// 4. Close the buffer PDA
/// 5. Close the state diff account
/// 6. Close the commit state record
/// 7. Close the delegation record
///
///
/// Accounts expected: Authority Record, Buffer PDA, Delegated PDA
pub fn process_undelegate<'a, 'info>(
    _program_id: &Pubkey,
    accounts: &'a [AccountInfo<'info>],
    _data: &[u8],
) -> ProgramResult {
    let [payer, delegated_account, owner_program, buffer, state_diff, commit_state_record, delegation_record, reimbursement, system_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    load_signer(payer)?;
    load_owned_pda(delegated_account, &crate::id())?;
    load_owned_pda(buffer, &crate::id())?;
    load_owned_pda(state_diff, &crate::id())?;
    load_owned_pda(commit_state_record, &crate::id())?;
    load_owned_pda(delegation_record, &crate::id())?;
    load_program(system_program, system_program::id())?;

    // Load delegation record
    let delegation_data = delegation_record.try_borrow_data()?;
    let delegation = Delegation::try_from_bytes(&delegation_data)?;

    let buffer_bump: u8 = load_initialized_pda(
        buffer,
        &[BUFFER, &delegated_account.key.to_bytes()],
        &crate::id(),
        true,
    )?;

    // Load committed state
    let commit_record_data = commit_state_record.try_borrow_data()?;
    let commit_record = CommitState::try_from_bytes(&commit_record_data)?;

    if !delegation.origin.eq(owner_program.key) {
        return Err(ProgramError::InvalidAccountData);
    }

    if !commit_record.account.eq(delegated_account.key) {
        return Err(ProgramError::InvalidAccountData);
    }

    // TODO: Add the logic to check the state diff, Authority and/or Fraud proof

    buffer.realloc(state_diff.data_len(), false)?;

    let mut buffer_data = buffer.try_borrow_mut_data()?;
    let new_data = state_diff.try_borrow_data()?;
    (*buffer_data).copy_from_slice(&new_data);

    msg!("Buffer PDA: {:?}", buffer.key.to_string());
    msg!("Buffer Data: {:?}", buffer_data.to_vec());

    // Dropping references
    drop(commit_record_data);
    drop(delegation_data);
    drop(buffer_data);
    drop(new_data);

    // Closing delegated account before reopening it with the original owner
    close_pda(delegated_account, reimbursement)?;

    // CPI to the owner program to re-open the PDA
    let signer_seeds: &[&[&[u8]]] = &[&[BUFFER, &delegated_account.key.to_bytes(), &[buffer_bump]]];
    call_external_undelegate(
        payer,
        delegated_account,
        buffer,
        system_program,
        owner_program.key,
        signer_seeds,
    )?;

    // Closing accounts
    close_pda(commit_state_record, reimbursement)?;
    close_pda(delegation_record, reimbursement)?;
    close_pda(state_diff, reimbursement)?;
    close_pda(buffer, reimbursement)?;
    Ok(())
}

fn call_external_undelegate<'a, 'info>(
    payer: &'a AccountInfo<'info>,
    account_to_undelegate: &'a AccountInfo<'info>,
    buffer: &'a AccountInfo<'info>,
    system_program: &'a AccountInfo<'info>,
    program_id: &Pubkey,
    signers_seeds: &[&[&[u8]]],
) -> ProgramResult {
    let instruction_data = EXTERNAL_UNDELEGATE_DISCRIMINATOR.to_vec();

    let close_instruction = Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta {
                pubkey: *account_to_undelegate.key,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: *buffer.key,
                is_signer: true,
                is_writable: true,
            },
            AccountMeta {
                pubkey: *payer.key,
                is_signer: true,
                is_writable: true,
            },
            AccountMeta {
                pubkey: *system_program.key,
                is_signer: false,
                is_writable: false,
            },
        ],
        data: instruction_data,
    };

    invoke_signed(
        &close_instruction,
        &[account_to_undelegate.clone(), payer.clone(), buffer.clone()],
        signers_seeds,
    )
}
