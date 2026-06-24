use dlp::solana_program;
use dlp_api::{
    error::DlpError,
    pda::{
        commit_record_pda_from_delegated_account,
        commit_state_pda_from_delegated_account,
        delegation_metadata_pda_from_delegated_account,
        delegation_record_pda_from_delegated_account,
        undelegation_request_pda_from_delegated_account,
    },
};
use solana_program::{hash::Hash, native_token::LAMPORTS_PER_SOL, rent::Rent};
use solana_program_test::{
    read_file, BanksClient, BanksClientError, ProgramTest,
};
use solana_sdk::{
    account::Account,
    instruction::InstructionError,
    signature::{Keypair, Signer},
    transaction::{Transaction, TransactionError},
};
use solana_sdk_ids::system_program;

use crate::fixtures::{
    create_undelegation_request_data_with_expiry,
    get_commit_record_account_data, get_delegation_metadata_data,
    get_delegation_record_data, keypair_from_bytes,
    COMMIT_NEW_STATE_ACCOUNT_DATA, DELEGATED_PDA, DELEGATED_PDA_ID,
    DELEGATED_PDA_OWNER_ID, TEST_AUTHORITY,
};

mod fixtures;

#[tokio::test]
async fn test_carry_over_requested_undelegation_after_expiry() {
    let (
        banks,
        caller,
        request_rent_payer,
        delegation_rent_payer,
        _,
        blockhash,
    ) = setup_carry_over_env(false, 0).await;

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
    let request_payer_before = banks
        .get_account(request_rent_payer.pubkey())
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

    let ix = dlp_api::instruction_builder::carry_over_requested_undelegation(
        caller.pubkey(),
        DELEGATED_PDA_ID,
        DELEGATED_PDA_OWNER_ID,
        request_rent_payer.pubkey(),
        delegation_rent_payer.pubkey(),
        caller.pubkey(),
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&caller.pubkey()),
        &[&caller],
        blockhash,
    );

    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok());

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

    let request_payer_after = banks
        .get_account(request_rent_payer.pubkey())
        .await
        .unwrap()
        .unwrap()
        .lamports;
    assert_eq!(request_payer_after, request_payer_before + request_lamports);

    let delegation_payer_after = banks
        .get_account(delegation_rent_payer.pubkey())
        .await
        .unwrap()
        .unwrap()
        .lamports;
    assert_eq!(
        delegation_payer_after,
        delegation_payer_before
            + delegation_record_lamports
            + delegation_metadata_lamports
    );
}

#[tokio::test]
async fn test_carry_over_requested_undelegation_rejects_before_expiry() {
    let (
        banks,
        caller,
        request_rent_payer,
        delegation_rent_payer,
        _,
        blockhash,
    ) = setup_carry_over_env(false, 1_000_000).await;

    let request_pda =
        undelegation_request_pda_from_delegated_account(&DELEGATED_PDA_ID);
    let delegation_record_pda =
        delegation_record_pda_from_delegated_account(&DELEGATED_PDA_ID);
    let delegation_metadata_pda =
        delegation_metadata_pda_from_delegated_account(&DELEGATED_PDA_ID);

    let request_before = banks.get_account(request_pda).await.unwrap().unwrap();
    let delegation_record_before = banks
        .get_account(delegation_record_pda)
        .await
        .unwrap()
        .unwrap();
    let delegation_metadata_before = banks
        .get_account(delegation_metadata_pda)
        .await
        .unwrap()
        .unwrap();
    let delegated_before =
        banks.get_account(DELEGATED_PDA_ID).await.unwrap().unwrap();

    let ix = dlp_api::instruction_builder::carry_over_requested_undelegation(
        caller.pubkey(),
        DELEGATED_PDA_ID,
        DELEGATED_PDA_OWNER_ID,
        request_rent_payer.pubkey(),
        delegation_rent_payer.pubkey(),
        caller.pubkey(),
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

    assert_eq!(
        banks.get_account(request_pda).await.unwrap().unwrap(),
        request_before
    );
    assert_eq!(
        banks
            .get_account(delegation_record_pda)
            .await
            .unwrap()
            .unwrap(),
        delegation_record_before
    );
    assert_eq!(
        banks
            .get_account(delegation_metadata_pda)
            .await
            .unwrap()
            .unwrap(),
        delegation_metadata_before
    );
    assert_eq!(
        banks.get_account(DELEGATED_PDA_ID).await.unwrap().unwrap(),
        delegated_before
    );
}

#[tokio::test]
async fn test_carry_over_closes_pending_commit_without_applying_it() {
    let (
        banks,
        caller,
        request_rent_payer,
        delegation_rent_payer,
        validator,
        blockhash,
    ) = setup_carry_over_env(true, 0).await;

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

    let ix = dlp_api::instruction_builder::carry_over_requested_undelegation(
        caller.pubkey(),
        DELEGATED_PDA_ID,
        DELEGATED_PDA_OWNER_ID,
        request_rent_payer.pubkey(),
        delegation_rent_payer.pubkey(),
        validator.pubkey(),
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&caller.pubkey()),
        &[&caller],
        blockhash,
    );

    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok());

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

async fn setup_carry_over_env(
    with_pending_commit: bool,
    expires_at_slot: u64,
) -> (BanksClient, Keypair, Keypair, Keypair, Keypair, Hash) {
    let mut program_test = ProgramTest::new("dlp", dlp_api::ID, None);
    program_test.prefer_bpf(true);

    let caller = Keypair::new();
    let request_rent_payer = Keypair::new();
    let delegation_rent_payer = Keypair::new();
    let validator = keypair_from_bytes(&TEST_AUTHORITY);

    add_system_account(&mut program_test, caller.pubkey());
    add_system_account(&mut program_test, request_rent_payer.pubkey());
    add_system_account(&mut program_test, delegation_rent_payer.pubkey());
    add_system_account(&mut program_test, validator.pubkey());
    add_delegated_account(&mut program_test);
    add_delegation_accounts(&mut program_test, delegation_rent_payer.pubkey());
    add_request_account(
        &mut program_test,
        request_rent_payer.pubkey(),
        expires_at_slot,
    );
    add_owner_program(&mut program_test);
    if with_pending_commit {
        add_pending_commit_accounts(&mut program_test, validator.pubkey());
    }

    let (banks, _, blockhash) = program_test.start().await;
    (
        banks,
        caller,
        request_rent_payer,
        delegation_rent_payer,
        validator,
        blockhash,
    )
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

    let delegation_metadata_data =
        get_delegation_metadata_data(delegation_rent_payer, Some(false));
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

fn add_request_account(
    program_test: &mut ProgramTest,
    request_rent_payer: solana_program::pubkey::Pubkey,
    expires_at_slot: u64,
) {
    let request_data = create_undelegation_request_data_with_expiry(
        DELEGATED_PDA_ID,
        DELEGATED_PDA_OWNER_ID,
        request_rent_payer,
        0,
        expires_at_slot,
        0,
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

fn add_owner_program(program_test: &mut ProgramTest) {
    let data = read_file("tests/buffers/test_delegation.so");
    program_test.add_account(
        DELEGATED_PDA_OWNER_ID,
        Account {
            lamports: Rent::default().minimum_balance(data.len()).max(1),
            data,
            owner: solana_sdk::bpf_loader::id(),
            executable: true,
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
