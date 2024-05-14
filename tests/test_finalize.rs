use solana_program::pubkey::Pubkey;
use solana_program::rent::Rent;
use solana_program::{hash::Hash, native_token::LAMPORTS_PER_SOL, system_program};
use solana_program_test::{processor, read_file, BanksClient, ProgramTest};
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

use dlp::consts::{COMMIT_RECORD, DELEGATION, STATE_DIFF};

use crate::fixtures::{
    COMMIT_NEW_STATE_ACCOUNT_DATA, COMMIT_STATE_AUTHORITY, COMMIT_STATE_RECORD_ACCOUNT_DATA,
    DELEGATED_PDA_ID, DELEGATED_PDA_OWNER_ID, DELEGATION_RECORD_ACCOUNT_DATA,
};

mod fixtures;

#[tokio::test]
async fn test_finalize() {
    // Setup
    let (mut banks, payer, _, blockhash) = setup_program_test_env().await;

    // Retrieve the accounts
    let delegation_record =
        Pubkey::find_program_address(&[DELEGATION, &DELEGATED_PDA_ID.to_bytes()], &dlp::id());
    let new_state_pda =
        Pubkey::find_program_address(&[STATE_DIFF, &DELEGATED_PDA_ID.to_bytes()], &dlp::id());
    let commit_state_record_pda = Pubkey::find_program_address(
        &[
            COMMIT_RECORD,
            &0u64.to_be_bytes(),
            &DELEGATED_PDA_ID.to_bytes(),
        ],
        &dlp::id(),
    );

    // Save the new state data before finalizing
    let new_state_before_finalize = banks.get_account(new_state_pda.0).await.unwrap().unwrap();
    let new_state_data_before_finalize = new_state_before_finalize.data.clone();

    // Submit the undelegate tx
    let ix = dlp::instruction::finalize(
        payer.pubkey(),
        DELEGATED_PDA_ID,
        new_state_pda.0,
        commit_state_record_pda.0,
        delegation_record.0,
        COMMIT_STATE_AUTHORITY,
    );
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[&payer], blockhash);
    let res = banks.process_transaction(tx).await;
    println!("{:?}", res);
    assert!(res.is_ok());

    // Assert the state_diff was closed
    let new_state_account = banks.get_account(new_state_pda.0).await.unwrap();
    assert!(new_state_account.is_none());

    // Assert the delegation_record was not closed
    let delegation_account = banks.get_account(delegation_record.0).await.unwrap();
    assert!(delegation_account.is_some());

    // Assert the commit_state_record_pda was closed
    let commit_state_record_account = banks.get_account(commit_state_record_pda.0).await.unwrap();
    assert!(commit_state_record_account.is_none());

    // Assert that the account owner is still the delegation program
    let pda_account = banks.get_account(DELEGATED_PDA_ID).await.unwrap().unwrap();
    assert!(pda_account.owner.eq(&dlp::id()));

    // Assert the delegated account contains the data from the new state
    assert_eq!(new_state_data_before_finalize, pda_account.data);
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
        Pubkey::find_program_address(&[DELEGATION, &DELEGATED_PDA_ID.to_bytes()], &dlp::id()).0,
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: DELEGATION_RECORD_ACCOUNT_DATA.into(),
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the commit state PDA
    program_test.add_account(
        Pubkey::find_program_address(&[STATE_DIFF, &DELEGATED_PDA_ID.to_bytes()], &dlp::id()).0,
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
        Pubkey::find_program_address(
            &[
                COMMIT_RECORD,
                &0u64.to_be_bytes(),
                &DELEGATED_PDA_ID.to_bytes(),
            ],
            &dlp::id(),
        )
        .0,
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: COMMIT_STATE_RECORD_ACCOUNT_DATA.into(),
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup program to test undelegation
    let data = read_file(&"tests/buffers/test_delegation.so");
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
