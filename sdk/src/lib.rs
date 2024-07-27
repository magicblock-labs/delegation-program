use borsh::BorshSerialize;
use solana_program::account_info::AccountInfo;
use solana_program::entrypoint::ProgramResult;
use solana_program::instruction::{AccountMeta, Instruction};
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;

pub use delegation_sdk_attribute_delegate::delegate;

// TODO: import from the delegation program crate once open-sourced
use crate::consts::BUFFER;
use crate::types::DelegateAccountArgs;
use crate::utils::{close_pda, create_pda, seeds_with_bump};
pub mod consts;
pub mod delegate_args;
pub mod pda;
pub mod types;
pub mod utils;

pub fn id() -> Pubkey {
    consts::DELEGATION_PROGRAM_ID
}

/// Delegate an account
#[allow(clippy::too_many_arguments)]
#[inline(always)]
pub fn delegate_account<'a, 'info>(
    payer: &'a AccountInfo<'info>,
    pda: &'a AccountInfo<'info>,
    owner_program: &'a AccountInfo<'info>,
    buffer: &'a AccountInfo<'info>,
    delegation_record: &'a AccountInfo<'info>,
    delegation_metadata: &'a AccountInfo<'info>,
    delegation_program: &'a AccountInfo<'info>,
    system_program: &'a AccountInfo<'info>,
    pda_seeds: &[&[u8]],
    valid_until: i64,
    commit_frequency_ms: u32,
) -> ProgramResult {
    let buffer_seeds: &[&[u8]] = &[BUFFER, pda.key.as_ref()];

    let (_, delegate_account_bump) = Pubkey::find_program_address(pda_seeds, owner_program.key);

    let (_, buffer_pda_bump) = Pubkey::find_program_address(buffer_seeds, owner_program.key);

    // Pda signer seeds
    let delegate_account_bump_slice: &[u8] = &[delegate_account_bump];
    let pda_signer_seeds: &[&[&[u8]]] =
        &[&*seeds_with_bump(pda_seeds, delegate_account_bump_slice)];

    // Buffer signer seeds
    let buffer_bump_slice: &[u8] = &[buffer_pda_bump];
    let buffer_signer_seeds: &[&[&[u8]]] = &[&*seeds_with_bump(buffer_seeds, buffer_bump_slice)];

    let data_len = pda.data_len();

    // Create the Buffer PDA
    create_pda(
        buffer,
        owner_program.key,
        data_len,
        buffer_signer_seeds,
        system_program,
        payer,
    )?;

    // Copy the date to the buffer PDA
    let mut buffer_data = buffer.try_borrow_mut_data()?;
    let new_data = pda.try_borrow_data()?.to_vec().clone();
    (*buffer_data).copy_from_slice(&new_data);
    drop(buffer_data);

    // Close the PDA account
    close_pda(pda, payer)?;

    // Re-create the PDA setting the delegation program as owner
    create_pda(
        pda,
        delegation_program.key,
        data_len,
        pda_signer_seeds,
        system_program,
        payer,
    )?;

    let seeds_vec: Vec<Vec<u8>> = pda_seeds.iter().map(|&slice| slice.to_vec()).collect();

    let delegation_args = DelegateAccountArgs {
        valid_until,
        commit_frequency_ms,
        seeds: seeds_vec,
    };

    cpi_delegate(
        payer,
        pda,
        owner_program,
        buffer,
        delegation_record,
        delegation_metadata,
        system_program,
        pda_signer_seeds,
        delegation_args,
    )?;

    close_pda(buffer, payer)?;
    Ok(())
}

/// Undelegate an account
#[inline(always)]
pub fn undelegate_account<'a, 'info>(
    delegated_account: &'a AccountInfo<'info>,
    owner_program: &Pubkey,
    buffer: &'a AccountInfo<'info>,
    payer: &'a AccountInfo<'info>,
    system_program: &'a AccountInfo<'info>,
    account_signer_seeds: Vec<Vec<u8>>,
) -> ProgramResult {
    if !buffer.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let account_seeds: Vec<&[u8]> = account_signer_seeds.iter().map(|v| v.as_slice()).collect();

    let (_, account_bump) = Pubkey::find_program_address(account_seeds.as_ref(), owner_program);

    // Account signer seeds
    let account_bump_slice: &[u8] = &[account_bump];
    let account_signer_seeds: &[&[&[u8]]] = &[&*seeds_with_bump(
        account_seeds.as_ref(),
        account_bump_slice,
    )];

    // Re-create the original PDA
    create_pda(
        delegated_account,
        owner_program,
        buffer.data_len(),
        account_signer_seeds,
        system_program,
        payer,
    )?;

    let mut data = delegated_account.try_borrow_mut_data()?;
    let buffer_data = buffer.try_borrow_data()?;
    (*data).copy_from_slice(&buffer_data);
    Ok(())
}

/// CPI to the delegation program to delegate the account
#[allow(clippy::too_many_arguments)]
#[inline(always)]
pub fn cpi_delegate<'a, 'info>(
    payer: &'a AccountInfo<'info>,
    delegate_account: &'a AccountInfo<'info>,
    owner_program: &'a AccountInfo<'info>,
    buffer: &'a AccountInfo<'info>,
    delegation_record: &'a AccountInfo<'info>,
    delegation_metadata: &'a AccountInfo<'info>,
    system_program: &'a AccountInfo<'info>,
    signers_seeds: &[&[&[u8]]],
    args: DelegateAccountArgs,
) -> ProgramResult {
    let mut data: Vec<u8> = vec![0u8; 8];
    let serialized_seeds = args.try_to_vec()?;
    data.extend_from_slice(&serialized_seeds);

    let delegation_instruction = Instruction {
        program_id: id(),
        accounts: vec![
            AccountMeta::new(*payer.key, true),
            AccountMeta::new(*delegate_account.key, true),
            AccountMeta::new_readonly(*owner_program.key, false),
            AccountMeta::new(*buffer.key, false),
            AccountMeta::new(*delegation_record.key, false),
            AccountMeta::new(*delegation_metadata.key, false),
            AccountMeta::new_readonly(*system_program.key, false),
        ],
        data,
    };

    solana_program::program::invoke_signed(
        &delegation_instruction,
        &[
            payer.clone(),
            delegate_account.clone(),
            owner_program.clone(),
            buffer.clone(),
            delegation_record.clone(),
            delegation_metadata.clone(),
            system_program.clone(),
        ],
        signers_seeds,
    )
}

/// CPI to the delegation program to allow undelegation
#[inline(always)]
pub fn allow_undelegation<'a, 'info>(
    delegated_account: &'a AccountInfo<'info>,
    delegation_record: &'a AccountInfo<'info>,
    delegation_metedata: &'a AccountInfo<'info>,
    buffer: &'a AccountInfo<'info>,
    delegation_program: &'a AccountInfo<'info>,
    owner_program: &Pubkey,
) -> ProgramResult {
    let buffer_seeds: &[&[u8]] = &[BUFFER, delegated_account.key.as_ref()];
    let (_, buffer_pda_bump) = Pubkey::find_program_address(buffer_seeds, owner_program);
    let buffer_bump_slice: &[u8] = &[buffer_pda_bump];
    let buffer_signer_seeds: &[&[&[u8]]] = &[&*seeds_with_bump(buffer_seeds, buffer_bump_slice)];

    let allow_undelegation_instruction = Instruction {
        program_id: *delegation_program.key,
        accounts: vec![
            AccountMeta::new_readonly(*delegated_account.key, false),
            AccountMeta::new_readonly(*delegation_record.key, false),
            AccountMeta::new(*delegation_metedata.key, false),
            AccountMeta::new_readonly(*buffer.key, true),
        ],
        data: vec![0x4, 0, 0, 0, 0, 0, 0, 0],
    };

    solana_program::program::invoke_signed(
        &allow_undelegation_instruction,
        &[
            delegated_account.clone(),
            delegation_record.clone(),
            delegation_metedata.clone(),
            buffer.clone(),
        ],
        buffer_signer_seeds,
    )
}

/// CPI to trigger a commit of an account in the ER
#[inline(always)]
pub fn trigger_commit<'a, 'info>(
    payer: &'a AccountInfo<'info>,
    delegated_account: &'a AccountInfo<'info>,
    magic_program: &'a AccountInfo<'info>,
) -> ProgramResult {
    let allow_undelegation_instruction = Instruction {
        program_id: *magic_program.key,
        accounts: vec![
            AccountMeta::new_readonly(*payer.key, true),
            AccountMeta::new_readonly(*delegated_account.key, false),
        ],
        data: vec![0x1, 0, 0, 0],
    };

    solana_program::program::invoke(
        &allow_undelegation_instruction,
        &[payer.clone(), delegated_account.clone()],
    )
}
