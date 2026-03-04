use dlp::{
    consts::{COMMIT_FEE_LAMPORTS, SESSION_FEE_LAMPORTS},
    pda::{
        delegation_metadata_pda_from_delegated_account,
        delegation_record_pda_from_delegated_account, fees_vault_pda,
        validator_fees_vault_pda_from_validator,
    },
};
use solana_program::{
    hash::Hash, native_token::LAMPORTS_PER_SOL, rent::Rent, system_program,
};
use solana_program_test::{read_file, BanksClient, ProgramTest};
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

use crate::fixtures::{
    create_delegation_metadata_data_with_nonce, get_delegation_record_data,
    DELEGATED_PDA_ID, DELEGATED_PDA_OWNER_ID, TEST_AUTHORITY,
};

mod fixtures;

/// Maximum number of commits a user can make (with empty seeds) before the
/// combined rent of delegation_record + delegation_metadata is fully consumed.
///
/// DelegationRecord:   96 bytes → 1,559,040 lamports
/// DelegationMetadata: 53 bytes → 1,259,760 lamports  (empty seeds)
/// total_lamports = 2,818,800
///
/// n * COMMIT_FEE_LAMPORTS + SESSION_FEE_LAMPORTS <= total_lamports
/// n * 100_000 + 300_000 <= 2_818_800  →  n <= 25
const MAX_COMMITS_BEFORE_RENT_EXHAUSTED: u64 = 25;

#[tokio::test]
async fn test_commit_fees_on_undelegation() {
    let (banks, _, validator, blockhash) = setup_program_test_env().await;
    let fees_vault_pda = fees_vault_pda();

    let fees_vault_before = banks
        .get_account(fees_vault_pda)
        .await
        .unwrap()
        .unwrap()
        .lamports;

    let delegation_record_data =
        get_delegation_record_data(validator.pubkey(), None);
    let delegation_metadata_data = create_delegation_metadata_data_with_nonce(
        validator.pubkey(),
        &[],
        true,
        101,
    );

    let record_rent =
        Rent::default().minimum_balance(delegation_record_data.len());
    let metadata_rent =
        Rent::default().minimum_balance(delegation_metadata_data.len());
    let expected_total_fees = (COMMIT_FEE_LAMPORTS * 100
        + SESSION_FEE_LAMPORTS)
        .min(record_rent + metadata_rent);
    let expected_fees_vault_fee = expected_total_fees / 10;

    let ix = dlp::instruction_builder::undelegate(
        validator.pubkey(),
        DELEGATED_PDA_ID,
        DELEGATED_PDA_OWNER_ID,
        validator.pubkey(),
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&validator.pubkey()),
        &[&validator],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok());

    let fees_vault_after = banks
        .get_account(fees_vault_pda)
        .await
        .unwrap()
        .unwrap()
        .lamports;
    assert_eq!(
        fees_vault_after,
        fees_vault_before + expected_fees_vault_fee
    );
}

// At MAX_COMMITS_BEFORE_RENT_EXHAUSTED (25) the full fee fits within rent; no capping.
#[tokio::test]
async fn test_commit_fees_fully_paid_at_max_commits() {
    // nonce = MAX + 1 → commit_count = MAX, last point where full fee fits
    let nonce = MAX_COMMITS_BEFORE_RENT_EXHAUSTED + 1;
    let (banks, _, validator, blockhash) =
        setup_program_test_env_with_nonce(nonce).await;
    let fees_vault_pda = fees_vault_pda();

    let fees_vault_before = banks
        .get_account(fees_vault_pda)
        .await
        .unwrap()
        .unwrap()
        .lamports;

    let delegation_record_data =
        get_delegation_record_data(validator.pubkey(), None);
    let delegation_metadata_data = create_delegation_metadata_data_with_nonce(
        validator.pubkey(),
        &[],
        true,
        nonce,
    );
    let record_rent =
        Rent::default().minimum_balance(delegation_record_data.len());
    let metadata_rent =
        Rent::default().minimum_balance(delegation_metadata_data.len());
    let total_lamports = record_rent + metadata_rent;

    let expected_total_fees =
        COMMIT_FEE_LAMPORTS * MAX_COMMITS_BEFORE_RENT_EXHAUSTED
            + SESSION_FEE_LAMPORTS;
    // The full fee fits within total_lamports — no capping
    assert!(
        expected_total_fees <= total_lamports,
        "fee {expected_total_fees} should fit in {total_lamports} lamports"
    );
    let expected_fees_vault_fee = expected_total_fees / 10;

    let ix = dlp::instruction_builder::undelegate(
        validator.pubkey(),
        DELEGATED_PDA_ID,
        DELEGATED_PDA_OWNER_ID,
        validator.pubkey(),
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&validator.pubkey()),
        &[&validator],
        blockhash,
    );
    assert!(banks.process_transaction(tx).await.is_ok());

    let fees_vault_after = banks
        .get_account(fees_vault_pda)
        .await
        .unwrap()
        .unwrap()
        .lamports;
    assert_eq!(
        fees_vault_after,
        fees_vault_before + expected_fees_vault_fee,
        "at {MAX_COMMITS_BEFORE_RENT_EXHAUSTED} commits the full fee ({expected_total_fees}) should be charged"
    );
}

// One commit past the max exhausts rent; fee is capped at total available lamports.
#[tokio::test]
async fn test_commit_fees_capped_one_past_max_commits() {
    // nonce = MAX + 2 → commit_count = MAX + 1, one past the limit
    let nonce = MAX_COMMITS_BEFORE_RENT_EXHAUSTED + 2;
    let (banks, _, validator, blockhash) =
        setup_program_test_env_with_nonce(nonce).await;
    let fees_vault_pda = fees_vault_pda();

    let fees_vault_before = banks
        .get_account(fees_vault_pda)
        .await
        .unwrap()
        .unwrap()
        .lamports;

    let delegation_record_data =
        get_delegation_record_data(validator.pubkey(), None);
    let delegation_metadata_data = create_delegation_metadata_data_with_nonce(
        validator.pubkey(),
        &[],
        true,
        nonce,
    );
    let record_rent =
        Rent::default().minimum_balance(delegation_record_data.len());
    let metadata_rent =
        Rent::default().minimum_balance(delegation_metadata_data.len());
    let total_lamports = record_rent + metadata_rent;

    // requested fee exceeds available lamports
    let fee_requested = COMMIT_FEE_LAMPORTS * (MAX_COMMITS_BEFORE_RENT_EXHAUSTED + 1)
        + SESSION_FEE_LAMPORTS;
    assert!(
        fee_requested > total_lamports,
        "fee {fee_requested} should exceed {total_lamports} lamports"
    );
    let expected_total_fees = fee_requested.min(total_lamports);
    // Assert: fee was capped — all rent is drained
    assert_eq!(expected_total_fees, total_lamports);
    let expected_fees_vault_fee = expected_total_fees / 10;

    let ix = dlp::instruction_builder::undelegate(
        validator.pubkey(),
        DELEGATED_PDA_ID,
        DELEGATED_PDA_OWNER_ID,
        validator.pubkey(),
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&validator.pubkey()),
        &[&validator],
        blockhash,
    );
    assert!(banks.process_transaction(tx).await.is_ok());

    let fees_vault_after = banks
        .get_account(fees_vault_pda)
        .await
        .unwrap()
        .unwrap()
        .lamports;
    // Assert: on-chain fee matches the cap (total_lamports), not the requested fee
    assert_eq!(
        fees_vault_after,
        fees_vault_before + expected_fees_vault_fee,
        "at {} commits the fee is capped at all available rent ({total_lamports})",
            MAX_COMMITS_BEFORE_RENT_EXHAUSTED + 1
    );
}

async fn setup_program_test_env() -> (BanksClient, Keypair, Keypair, Hash) {
    setup_program_test_env_with_nonce(101).await
}

async fn setup_program_test_env_with_nonce(
    last_update_nonce: u64,
) -> (BanksClient, Keypair, Keypair, Hash) {
    let mut program_test = ProgramTest::new("dlp", dlp::ID, None);
    program_test.prefer_bpf(true);
    let validator = Keypair::from_bytes(&TEST_AUTHORITY).unwrap();

    program_test.add_account(
        validator.pubkey(),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup a delegated PDA
    program_test.add_account(
        DELEGATED_PDA_ID,
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the delegated record PDA
    let delegation_record_data =
        get_delegation_record_data(validator.pubkey(), None);
    program_test.add_account(
        delegation_record_pda_from_delegated_account(&DELEGATED_PDA_ID),
        Account {
            lamports: Rent::default()
                .minimum_balance(delegation_record_data.len()),
            data: delegation_record_data,
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the delegated account metadata PDA with the given nonce
    let delegation_metadata_data = create_delegation_metadata_data_with_nonce(
        validator.pubkey(),
        &[],
        true,
        last_update_nonce,
    );
    program_test.add_account(
        delegation_metadata_pda_from_delegated_account(&DELEGATED_PDA_ID),
        Account {
            lamports: Rent::default()
                .minimum_balance(delegation_metadata_data.len()),
            data: delegation_metadata_data,
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup program to test undelegation
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

    // Setup the protocol fees vault
    program_test.add_account(
        fees_vault_pda(),
        Account {
            lamports: Rent::default().minimum_balance(0),
            data: vec![],
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the validator fees vault
    program_test.add_account(
        validator_fees_vault_pda_from_validator(&validator.pubkey()),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks, payer, blockhash) = program_test.start().await;
    (banks, payer, validator, blockhash)
}
