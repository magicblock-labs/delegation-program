use solana_program::{{self}, account_info::AccountInfo, entrypoint::ProgramResult, msg, pubkey::Pubkey};
use solana_program::instruction::{AccountMeta, Instruction};
use solana_program::program::invoke;
use solana_program::program_error::ProgramError;
use crate::instruction::commit_state;

use crate::loaders::load_owned_pda;
use crate::state::{CommitState, Delegation};
use crate::utils::{AccountDeserialize, close_pda};

/// Undelegate a delegated Pda
///
/// 1. If the state diff is valid, copy the committed state to the buffer PDA
/// 2. Close the locked account
/// 3. Close the state diff account
/// 4. CPI to the original owner to re-open the PDA with the original owner and the new state
/// 5. Close the buffer PDA
/// 6. Close the delegation record
///
///
/// Accounts expected: Authority Record, Buffer PDA, Delegated PDA
pub fn process_undelegate<'a, 'info>(
    _program_id: &Pubkey,
    accounts: &'a [AccountInfo<'info>],
    data: &[u8],
) -> ProgramResult {
    msg!("Processing delegate instruction");
    msg!("Data: {:?}", data);
    let [ delegated_account, owner_program, buffer, state_diff, commit_state_record, delegation_record, reimbursement] =
        accounts
        else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };
    msg!("Load accounts");
    load_owned_pda(delegated_account, &crate::id())?;
    load_owned_pda(buffer, &crate::id())?;
    load_owned_pda(state_diff, &crate::id())?;
    load_owned_pda(commit_state_record, &crate::id())?;
    load_owned_pda(delegation_record, &crate::id())?;

    // Load delegation record
    let delegation_data = delegation_record.try_borrow_data()?;
    let delegation = Delegation::try_from_bytes(&delegation_data)?;

    // Load committed state
    let commit_record_data = commit_state_record.try_borrow_data()?;
    let commit_record = CommitState::try_from_bytes(&commit_record_data)?;

    if !delegation.origin.eq(owner_program.key) {
        return Err(ProgramError::InvalidAccountData);
    }

    if !commit_record.account.eq(delegated_account.key) {
        return Err(ProgramError::InvalidAccountData);
    }

    if !commit_record.identity.eq(reimbursement.key){
        return Err(ProgramError::InvalidAccountData);
    }

    // TODO: Add the logic to check the state diff, Authority & Fraud proof


    // Close the delegated account
    close_pda(delegated_account, reimbursement)?;

    // TODO: CPI owner program to repoen the PDA with the original owner and the new state

    // Dropping references
    drop(commit_record_data);
    drop(delegation_data);

    // Closing accounts
    close_pda(commit_state_record, reimbursement)?;
    close_pda(delegation_record, reimbursement)?;
    close_pda(state_diff, reimbursement)?;
    close_pda(buffer, reimbursement)?;

    Ok(())
}

const CLOSE_INSTRUCTION_DISCRIMINATOR: [u8; 8] = [98, 165, 201, 177, 108, 65, 206, 96];

fn call_close_pda<'a, 'info>(
    account_to_close: &'a AccountInfo<'info>,
    destination_account: &'a AccountInfo<'info>,
    program_id: &Pubkey, // Anchor program's ID
) -> ProgramResult {
    let instruction_data = CLOSE_INSTRUCTION_DISCRIMINATOR.to_vec();

    let close_instruction = Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta {
                pubkey: *account_to_close.key,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: *account_to_close.key,
                is_signer: false,
                is_writable: true,
            },
        ],
        data: instruction_data,
    };

    invoke(
        &close_instruction,
        &[account_to_close.clone(), destination_account.clone()],
    )
}
