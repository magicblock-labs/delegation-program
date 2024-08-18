use borsh::BorshDeserialize;
use dlp::pda::{
    committed_state_pda_from_pubkey, committed_state_record_pda_from_pubkey,
    delegation_metadata_pda_from_pubkey, delegation_record_pda_from_pubkey,
};
use dlp::state::{CommitRecord, DelegationMetadata};
use dlp::utils_account::AccountDeserialize;
use solana_program::rent::Rent;
use solana_program::{hash::Hash, native_token::LAMPORTS_PER_SOL, system_program};
use solana_program_test::{processor, read_file, BanksClient, ProgramTest};
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

use crate::fixtures::{
    COMMIT_NEW_STATE_ACCOUNT_DATA, COMMIT_STATE_AUTHORITY, COMMIT_STATE_RECORD_ACCOUNT_DATA,
    DELEGATED_PDA_ID, DELEGATED_PDA_OWNER_ID, DELEGATION_METADATA_PDA,
    DELEGATION_RECORD_ACCOUNT_DATA,
};

mod fixtures;

#[tokio::test]
async fn test_finalize() {
    // Setup
    let (mut banks, payer, _, blockhash) = setup_program_test_env().await;

    // Retrieve the accounts
    let delegation_record = delegation_record_pda_from_pubkey(&DELEGATED_PDA_ID);
    let committed_state_pda = committed_state_pda_from_pubkey(&DELEGATED_PDA_ID);
    let commit_state_record_pda = committed_state_record_pda_from_pubkey(&DELEGATED_PDA_ID);

    // Commit state record data
    let commit_state_record = banks
        .get_account(commit_state_record_pda)
        .await
        .unwrap()
        .unwrap();
    let commit_state_record = CommitRecord::try_from_bytes(&commit_state_record.data).unwrap();

    // Save the new state data before finalizing
    let new_state_before_finalize = banks
        .get_account(committed_state_pda)
        .await
        .unwrap()
        .unwrap();
    let new_state_data_before_finalize = new_state_before_finalize.data.clone();

    // Submit the undelegate tx
    let ix = dlp::instruction::finalize(payer.pubkey(), DELEGATED_PDA_ID, COMMIT_STATE_AUTHORITY);
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[&payer], blockhash);
    let res = banks.process_transaction(tx).await;
    println!("{:?}", res);
    assert!(res.is_ok());

    // Assert the state_diff was closed
    let committed_state_account = banks.get_account(committed_state_pda).await.unwrap();
    assert!(committed_state_account.is_none());

    // Assert the delegation_record was not closed
    let delegation_record = banks.get_account(delegation_record).await.unwrap();
    assert!(delegation_record.is_some());

    // Assert the commit_state_record_pda was closed
    let commit_state_record_account = banks.get_account(commit_state_record_pda).await.unwrap();
    assert!(commit_state_record_account.is_none());

    // Assert that the account owner is still the delegation program
    let pda_account = banks.get_account(DELEGATED_PDA_ID).await.unwrap().unwrap();
    assert!(pda_account.owner.eq(&dlp::id()));

    // Assert the delegated account contains the data from the new state
    assert_eq!(new_state_data_before_finalize, pda_account.data);

    // Assert the delegation metadata contains the correct slot of the commitment
    let delegation_metadata_pda = delegation_metadata_pda_from_pubkey(&DELEGATED_PDA_ID);
    let delegation_metadata_account = banks
        .get_account(delegation_metadata_pda)
        .await
        .unwrap()
        .unwrap();
    let delegation_metadata =
        DelegationMetadata::try_from_slice(&delegation_metadata_account.data).unwrap();
    assert_eq!(
        commit_state_record.slot,
        delegation_metadata.last_update_external_slot
    );
}

async fn setup_program_test_env() -> (BanksClient, Keypair, Keypair, Hash) {
    let mut program_test = ProgramTest::new("dlp", dlp::ID, processor!(dlp::process_instruction));
    program_test.prefer_bpf(true);
    let payer_alt = Keypair::new();

    program_test.add_account(
        payer_alt.pubkey(),
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
    program_test.add_account(
        delegation_record_pda_from_pubkey(&DELEGATED_PDA_ID),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: DELEGATION_RECORD_ACCOUNT_DATA.into(),
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the delegated account metadata PDA
    program_test.add_account(
        delegation_metadata_pda_from_pubkey(&DELEGATED_PDA_ID),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: DELEGATION_METADATA_PDA.into(),
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the commit state PDA
    program_test.add_account(
        committed_state_pda_from_pubkey(&DELEGATED_PDA_ID),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: COMMIT_NEW_STATE_ACCOUNT_DATA.into(),
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the commit state record PDA
    program_test.add_account(
        committed_state_record_pda_from_pubkey(&DELEGATED_PDA_ID),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: COMMIT_STATE_RECORD_ACCOUNT_DATA.into(),
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

    let (banks, payer, blockhash) = program_test.start().await;
    (banks, payer, payer_alt, blockhash)
}
