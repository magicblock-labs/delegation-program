use dlp::solana_program;
use dlp_api::{
    consts::EXTERNAL_UNDELEGATE_DISCRIMINATOR,
    error::DlpError,
    pda::{
        commit_record_pda_from_delegated_account,
        commit_state_pda_from_delegated_account,
        delegation_metadata_pda_from_delegated_account,
        delegation_record_pda_from_delegated_account,
        undelegation_request_pda_from_delegated_account,
    },
};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    hash::Hash,
    instruction::{AccountMeta, Instruction},
    native_token::LAMPORTS_PER_SOL,
    program::invoke_signed,
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::Sysvar,
};
use solana_program_test::{
    processor, BanksClient, BanksClientError, ProgramTest,
};
use solana_sdk::{
    account::Account,
    instruction::InstructionError,
    signature::{Keypair, Signer},
    transaction::{Transaction, TransactionError},
};
use solana_sdk_ids::system_program;
use solana_system_interface::instruction as system_instruction;

use crate::fixtures::{
    create_undelegation_request_data_with_expiry,
    get_commit_record_account_data,
    get_delegation_metadata_data_with_commit_id, get_delegation_record_data,
    keypair_from_bytes, COMMIT_NEW_STATE_ACCOUNT_DATA, DELEGATED_PDA,
    DELEGATED_PDA_ID, DELEGATED_PDA_OWNER_ID, TEST_AUTHORITY,
};

mod fixtures;

const TEST_PDA_SEED: &[u8] = b"test-pda";

#[tokio::test]
async fn test_undelegate_with_rollback_after_timeout_after_expiry() {
    let (banks, caller, delegation_rent_payer, _, blockhash) =
        setup_request_timeout_env(false, 0).await;

    let request_pda =
        undelegation_request_pda_from_delegated_account(&DELEGATED_PDA_ID);
    let delegation_record_pda =
        delegation_record_pda_from_delegated_account(&DELEGATED_PDA_ID);
    let delegation_metadata_pda =
        delegation_metadata_pda_from_delegated_account(&DELEGATED_PDA_ID);

    let request_lamports = banks
        .get_account(request_pda)
        .await
        .unwrap()
        .unwrap()
        .lamports;
    let delegation_record_lamports = banks
        .get_account(delegation_record_pda)
        .await
        .unwrap()
        .unwrap()
        .lamports;
    let delegation_metadata_lamports = banks
        .get_account(delegation_metadata_pda)
        .await
        .unwrap()
        .unwrap()
        .lamports;
    let delegation_payer_before = banks
        .get_account(delegation_rent_payer.pubkey())
        .await
        .unwrap()
        .unwrap()
        .lamports;
    let delegated_before =
        banks.get_account(DELEGATED_PDA_ID).await.unwrap().unwrap();

    let ix = rollback_from_owner_program(
        delegation_rent_payer.pubkey(),
        delegation_rent_payer.pubkey(),
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&caller.pubkey()),
        &[&caller],
        blockhash,
    );

    banks.process_transaction(tx).await.unwrap();

    assert!(banks.get_account(request_pda).await.unwrap().is_none());
    assert!(banks
        .get_account(delegation_record_pda)
        .await
        .unwrap()
        .is_none());
    assert!(banks
        .get_account(delegation_metadata_pda)
        .await
        .unwrap()
        .is_none());

    let delegated_after =
        banks.get_account(DELEGATED_PDA_ID).await.unwrap().unwrap();
    assert_eq!(delegated_after.owner, DELEGATED_PDA_OWNER_ID);
    assert_eq!(delegated_after.data, delegated_before.data);

    let delegation_payer_after = banks
        .get_account(delegation_rent_payer.pubkey())
        .await
        .unwrap()
        .unwrap()
        .lamports;
    assert_eq!(
        delegation_payer_after,
        delegation_payer_before
            + request_lamports
            + delegation_record_lamports
            + delegation_metadata_lamports
    );
}

#[tokio::test]
async fn test_undelegate_with_rollback_after_timeout_rejects_before_expiry() {
    let (banks, caller, delegation_rent_payer, _, blockhash) =
        setup_request_timeout_env(false, 1_000_000).await;

    let ix = rollback_from_owner_program(
        delegation_rent_payer.pubkey(),
        delegation_rent_payer.pubkey(),
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&caller.pubkey()),
        &[&caller],
        blockhash,
    );

    let err = banks.process_transaction(tx).await.unwrap_err();
    assert!(
        matches!(
            err,
            BanksClientError::TransactionError(
                TransactionError::InstructionError(
                    0,
                    InstructionError::Custom(code),
                )
            ) if code == DlpError::UndelegationRequestNotExpired as u32
        ),
        "expected UndelegationRequestNotExpired, got {err:?}"
    );
}

#[tokio::test]
async fn test_request_timeout_closes_pending_commit_without_applying_it() {
    let (banks, caller, delegation_rent_payer, validator, blockhash) =
        setup_request_timeout_env(true, 0).await;

    let commit_state_pda =
        commit_state_pda_from_delegated_account(&DELEGATED_PDA_ID);
    let commit_record_pda =
        commit_record_pda_from_delegated_account(&DELEGATED_PDA_ID);
    let commit_state_lamports = banks
        .get_account(commit_state_pda)
        .await
        .unwrap()
        .unwrap()
        .lamports;
    let commit_record_lamports = banks
        .get_account(commit_record_pda)
        .await
        .unwrap()
        .unwrap()
        .lamports;
    let validator_before = banks
        .get_account(validator.pubkey())
        .await
        .unwrap()
        .unwrap()
        .lamports;

    let ix = rollback_from_owner_program(
        delegation_rent_payer.pubkey(),
        validator.pubkey(),
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&caller.pubkey()),
        &[&caller],
        blockhash,
    );

    banks.process_transaction(tx).await.unwrap();

    assert!(banks.get_account(commit_state_pda).await.unwrap().is_none());
    assert!(banks
        .get_account(commit_record_pda)
        .await
        .unwrap()
        .is_none());

    let delegated_after =
        banks.get_account(DELEGATED_PDA_ID).await.unwrap().unwrap();
    assert_eq!(delegated_after.owner, DELEGATED_PDA_OWNER_ID);
    assert_eq!(delegated_after.data, DELEGATED_PDA);
    assert_ne!(delegated_after.data, COMMIT_NEW_STATE_ACCOUNT_DATA);

    let validator_after = banks
        .get_account(validator.pubkey())
        .await
        .unwrap()
        .unwrap()
        .lamports;
    assert_eq!(
        validator_after,
        validator_before + commit_state_lamports + commit_record_lamports
    );
}

async fn setup_request_timeout_env(
    with_pending_commit: bool,
    expires_at_slot: u64,
) -> (BanksClient, Keypair, Keypair, Keypair, Hash) {
    let mut program_test = ProgramTest::default();
    program_test.prefer_bpf(true);
    program_test.add_program("dlp", dlp_api::ID, None);
    program_test.prefer_bpf(false);
    program_test.add_program(
        "rollback-wrapper",
        DELEGATED_PDA_OWNER_ID,
        processor!(owner_program_processor),
    );
    program_test.prefer_bpf(true);

    let caller = Keypair::new();
    let delegation_rent_payer = Keypair::new();
    let validator = keypair_from_bytes(&TEST_AUTHORITY);

    add_system_account(&mut program_test, caller.pubkey());
    add_system_account(&mut program_test, delegation_rent_payer.pubkey());
    add_system_account(&mut program_test, validator.pubkey());
    add_delegated_account(&mut program_test);
    add_delegation_accounts(&mut program_test, delegation_rent_payer.pubkey());
    add_request_account(&mut program_test, expires_at_slot);
    if with_pending_commit {
        add_pending_commit_accounts(&mut program_test, validator.pubkey());
    }

    let (banks, _, blockhash) = program_test.start().await;
    (banks, caller, delegation_rent_payer, validator, blockhash)
}

fn add_system_account(
    program_test: &mut ProgramTest,
    pubkey: solana_program::pubkey::Pubkey,
) {
    program_test.add_account(
        pubkey,
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
}

fn add_delegated_account(program_test: &mut ProgramTest) {
    program_test.add_account(
        DELEGATED_PDA_ID,
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: DELEGATED_PDA.into(),
            owner: dlp_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
}

fn add_delegation_accounts(
    program_test: &mut ProgramTest,
    delegation_rent_payer: solana_program::pubkey::Pubkey,
) {
    let delegation_record_data = get_delegation_record_data(
        keypair_from_bytes(&TEST_AUTHORITY).pubkey(),
        None,
    );
    program_test.add_account(
        delegation_record_pda_from_delegated_account(&DELEGATED_PDA_ID),
        Account {
            lamports: Rent::default()
                .minimum_balance(delegation_record_data.len()),
            data: delegation_record_data,
            owner: dlp_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let delegation_metadata_data = get_delegation_metadata_data_with_commit_id(
        delegation_rent_payer,
        Some(false),
        0,
    );
    program_test.add_account(
        delegation_metadata_pda_from_delegated_account(&DELEGATED_PDA_ID),
        Account {
            lamports: Rent::default()
                .minimum_balance(delegation_metadata_data.len()),
            data: delegation_metadata_data,
            owner: dlp_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
}

fn add_request_account(program_test: &mut ProgramTest, expires_at_slot: u64) {
    let request_data = create_undelegation_request_data_with_expiry(
        DELEGATED_PDA_ID,
        expires_at_slot,
    );
    program_test.add_account(
        undelegation_request_pda_from_delegated_account(&DELEGATED_PDA_ID),
        Account {
            lamports: Rent::default().minimum_balance(request_data.len()),
            data: request_data,
            owner: dlp_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
}

fn add_pending_commit_accounts(
    program_test: &mut ProgramTest,
    validator: solana_program::pubkey::Pubkey,
) {
    program_test.add_account(
        commit_state_pda_from_delegated_account(&DELEGATED_PDA_ID),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: COMMIT_NEW_STATE_ACCOUNT_DATA.into(),
            owner: dlp_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let commit_record_data = get_commit_record_account_data(validator);
    program_test.add_account(
        commit_record_pda_from_delegated_account(&DELEGATED_PDA_ID),
        Account {
            lamports: Rent::default().minimum_balance(commit_record_data.len()),
            data: commit_record_data,
            owner: dlp_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
}

fn rollback_from_owner_program(
    delegation_rent_payer: Pubkey,
    commit_reimbursement: Pubkey,
) -> Instruction {
    Instruction {
        program_id: DELEGATED_PDA_OWNER_ID,
        accounts: vec![
            AccountMeta::new(DELEGATED_PDA_ID, false),
            AccountMeta::new_readonly(DELEGATED_PDA_OWNER_ID, false),
            AccountMeta::new(
                undelegation_request_pda_from_delegated_account(
                    &DELEGATED_PDA_ID,
                ),
                false,
            ),
            AccountMeta::new(
                delegation_record_pda_from_delegated_account(&DELEGATED_PDA_ID),
                false,
            ),
            AccountMeta::new(
                delegation_metadata_pda_from_delegated_account(
                    &DELEGATED_PDA_ID,
                ),
                false,
            ),
            AccountMeta::new(delegation_rent_payer, false),
            AccountMeta::new(
                commit_state_pda_from_delegated_account(&DELEGATED_PDA_ID),
                false,
            ),
            AccountMeta::new(
                commit_record_pda_from_delegated_account(&DELEGATED_PDA_ID),
                false,
            ),
            AccountMeta::new(commit_reimbursement, false),
            AccountMeta::new_readonly(dlp_api::id(), false),
        ],
        data: vec![0],
    }
}

fn owner_program_processor(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    if data.starts_with(&EXTERNAL_UNDELEGATE_DISCRIMINATOR) {
        return process_external_undelegate(program_id, accounts);
    }

    match data.first().copied() {
        Some(0) => process_rollback(program_id, accounts),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}

fn process_rollback(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let [delegated_account, owner_program, request_account, delegation_record_account, delegation_metadata_account, delegation_rent_payer, commit_state_account, commit_record_account, commit_reimbursement, dlp_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if owner_program.key != program_id {
        return Err(ProgramError::IncorrectProgramId);
    }

    let delegated_data = delegated_account.try_borrow_data()?.to_vec();

    let ix =
        dlp_api::instruction_builder::undelegate_with_rollback_after_timeout(
            *delegated_account.key,
            *owner_program.key,
            *delegation_rent_payer.key,
            *commit_reimbursement.key,
        );
    let (_, bump) = Pubkey::find_program_address(&[TEST_PDA_SEED], program_id);
    let bump_seed = [bump];
    invoke_signed(
        &ix,
        &[
            delegated_account.clone(),
            owner_program.clone(),
            request_account.clone(),
            delegation_record_account.clone(),
            delegation_metadata_account.clone(),
            delegation_rent_payer.clone(),
            commit_state_account.clone(),
            commit_record_account.clone(),
            commit_reimbursement.clone(),
            dlp_program.clone(),
        ],
        &[&[TEST_PDA_SEED, &bump_seed]],
    )?;

    delegated_account.resize(delegated_data.len())?;
    delegated_account
        .try_borrow_mut_data()?
        .copy_from_slice(&delegated_data);

    Ok(())
}

fn process_external_undelegate(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let [delegated_account, undelegate_buffer_account, payer, system_program_account] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    let space = undelegate_buffer_account.data_len();
    let rent_lamports = Rent::get()?.minimum_balance(space);
    let (_, bump) = Pubkey::find_program_address(&[TEST_PDA_SEED], program_id);
    let bump_seed = [bump];
    invoke_signed(
        &system_instruction::create_account(
            payer.key,
            delegated_account.key,
            rent_lamports,
            space as u64,
            program_id,
        ),
        &[
            payer.clone(),
            delegated_account.clone(),
            system_program_account.clone(),
        ],
        &[&[TEST_PDA_SEED, &bump_seed]],
    )?;

    let buffer_data = undelegate_buffer_account.try_borrow_data()?;
    let mut delegated_data = delegated_account.try_borrow_mut_data()?;
    delegated_data.copy_from_slice(&buffer_data);
    Ok(())
}
