use dlp::solana_program;
use dlp_api::{
    consts::{COMMIT_FEE_LAMPORTS, SESSION_FEE_LAMPORTS},
    pda::{
        delegation_metadata_pda_from_delegated_account,
        delegation_record_pda_from_delegated_account, fees_vault_pda,
        validator_fees_vault_pda_from_validator,
    },
};
use solana_program::{hash::Hash, native_token::LAMPORTS_PER_SOL, rent::Rent};
use solana_program_test::{read_file, BanksClient, ProgramTest};
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use solana_sdk_ids::system_program;

use crate::fixtures::{
    create_delegation_metadata_data_with_commit_id, get_delegation_record_data,
    DELEGATED_PDA_ID, DELEGATED_PDA_OWNER_ID, TEST_AUTHORITY,
};

mod fixtures;

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
    let delegation_metadata_data =
        create_delegation_metadata_data_with_commit_id(
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

    let ix = dlp_api::instruction_builder::undelegate(
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

async fn setup_program_test_env() -> (BanksClient, Keypair, Keypair, Hash) {
    let mut program_test = ProgramTest::new("dlp", dlp_api::ID, None);
    program_test.prefer_bpf(true);
    let validator = crate::fixtures::keypair_from_bytes(&TEST_AUTHORITY);

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
            owner: dlp_api::id(),
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
            owner: dlp_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the delegated account metadata PDA with high commit id
    let delegation_metadata_data =
        create_delegation_metadata_data_with_commit_id(
            validator.pubkey(),
            &[],
            true,
            101,
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
            owner: dlp_api::id(),
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
            owner: dlp_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks, payer, blockhash) = program_test.start().await;
    (banks, payer, validator, blockhash)
}
