use dlp::args::CommitFinalizeArgs;
use dlp::pda::{
    delegation_metadata_pda_from_delegated_account, delegation_record_pda_from_delegated_account,
    validator_fees_vault_pda_from_validator,
};
use dlp::state::DelegationMetadata;
use solana_program::rent::Rent;
use solana_program::{hash::Hash, native_token::LAMPORTS_PER_SOL, system_program};
use solana_program_test::{BanksClient, BanksTransactionResultWithMetadata, ProgramTest};
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

use crate::fixtures::{
    get_delegation_metadata_data, get_delegation_record_data, DELEGATED_PDA_ID,
    DELEGATED_PDA_OWNER_ID, TEST_AUTHORITY,
};

mod fixtures;

#[tokio::test]
async fn test_commit_finalize_perf() {
    // Setup
    let (banks, _, authority, blockhash) = setup_program_test_env(vec![0; 10240]).await;
    let new_state: Vec<u8> = vec![1; 10240];

    let new_account_balance = 1_000_000;

    let (ix, pdas) = dlp::instruction_builder::commit_finalize(
        authority.pubkey(),
        DELEGATED_PDA_ID,
        DELEGATED_PDA_OWNER_ID,
        &mut CommitFinalizeArgs {
            commit_id: 1,
            allow_undelegation: true.into(),
            data_is_diff: false.into(),
            lamports: new_account_balance,
            bumps: Default::default(),
            reserved_padding: Default::default(),
        },
        &new_state,
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&authority.pubkey()),
        &[&authority],
        blockhash,
    );

    // execute CommitFinalize and validate CU performmace
    {
        let BanksTransactionResultWithMetadata {
            result: _,
            metadata,
        } = banks.process_transaction_with_metadata(tx).await.unwrap();

        let metadata = metadata.unwrap();

        assertables::assert_lt!(metadata.compute_units_consumed, 1100);

        assert_eq!(
            metadata.log_messages.len(),
            3,
            "CommitFinalize must not log anything in OK scenario"
        );
    }

    let delegated_account = banks.get_account(DELEGATED_PDA_ID).await.unwrap().unwrap();
    assert_eq!(delegated_account.data, new_state);

    let delegation_metadata_account = banks
        .get_account(pdas.delegation_metadata)
        .await
        .unwrap()
        .unwrap();

    let delegation_metadata =
        DelegationMetadata::try_from_bytes_with_discriminator(&delegation_metadata_account.data)
            .unwrap();

    assert_eq!(delegation_metadata.is_undelegatable, true);
}

#[tokio::test]
async fn test_commit_finalize_out_of_order() {
    // Setup
    let (banks, _, authority, blockhash) = setup_program_test_env(vec![]).await;
    let new_state = vec![0, 1, 2, 9, 9, 9, 6, 7, 8, 9];

    let new_account_balance = 1_000_000;

    let (ix, _pdas) = dlp::instruction_builder::commit_finalize(
        authority.pubkey(),
        DELEGATED_PDA_ID,
        DELEGATED_PDA_OWNER_ID,
        &mut CommitFinalizeArgs {
            commit_id: 2, // this is the min value which will cause NonceOutOfOrder
            allow_undelegation: true.into(),
            data_is_diff: false.into(),
            lamports: new_account_balance,
            bumps: Default::default(),
            reserved_padding: Default::default(),
        },
        &new_state,
    );

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&authority.pubkey()),
        &[&authority],
        blockhash,
    );

    let BanksTransactionResultWithMetadata { result, metadata } =
        banks.process_transaction_with_metadata(tx).await.unwrap();

    let metadata = metadata.unwrap();

    let log = metadata
        .log_messages
        .iter()
        .find(|log| log.contains("require"))
        .unwrap();

    assert_eq!(
        log,
        "Program log: require_eq!(args.commit_id, prev_id + 1) failed: 2 == 1"
    );

    assert!(
        metadata
            .log_messages
            .iter()
            .any(|log| log.contains("NonceOutOfOrder")),
        "{:#?}",
        metadata
    );

    assert_eq!(
        result.unwrap_err().to_string(),
        "Error processing Instruction 0: custom program error: 0xc"
    );
}

async fn setup_program_test_env(pda_data: Vec<u8>) -> (BanksClient, Keypair, Keypair, Hash) {
    let mut program_test = ProgramTest::new("dlp", dlp::ID, None);
    program_test.prefer_bpf(true);

    let validator_keypair = Keypair::from_bytes(&TEST_AUTHORITY).unwrap();

    program_test.add_account(
        validator_keypair.pubkey(),
        Account {
            lamports: 10 * LAMPORTS_PER_SOL,
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
            data: pda_data,
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the delegated account metadata PDA
    let delegation_metadata_data = get_delegation_metadata_data(validator_keypair.pubkey(), None);
    program_test.add_account(
        delegation_metadata_pda_from_delegated_account(&DELEGATED_PDA_ID),
        Account {
            lamports: Rent::default().minimum_balance(delegation_metadata_data.len()),
            data: delegation_metadata_data,
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the delegated record PDA
    let delegation_record_data = get_delegation_record_data(validator_keypair.pubkey(), None);
    program_test.add_account(
        delegation_record_pda_from_delegated_account(&DELEGATED_PDA_ID),
        Account {
            lamports: Rent::default().minimum_balance(delegation_record_data.len()),
            data: delegation_record_data,
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the validator fees vault
    program_test.add_account(
        validator_fees_vault_pda_from_validator(&validator_keypair.pubkey()),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks, payer, blockhash) = program_test.start().await;
    (banks, payer, validator_keypair, blockhash)
}
