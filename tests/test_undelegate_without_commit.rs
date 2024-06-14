use solana_program::rent::Rent;
use solana_program::{hash::Hash, native_token::LAMPORTS_PER_SOL, system_program};
use solana_program_test::{processor, read_file, BanksClient, ProgramTest};
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

use dlp::pda::{
    committed_state_pda_from_pubkey, delegated_account_seeds_pda_from_pubkey,
    delegation_record_pda_from_pubkey,
};

use crate::fixtures::{
    DELEGATED_ACCOUNT_SEEDS_PDA, DELEGATED_PDA_ID, DELEGATED_PDA_OWNER_ID,
    DELEGATION_RECORD_ACCOUNT_DATA,
};

mod fixtures;

#[tokio::test]
async fn test_undelegate_without_commit() {
    // Setup
    let (mut banks, payer, _, blockhash) = setup_program_test_env().await;

    // Retrieve the accounts
    let delegation_pda = delegation_record_pda_from_pubkey(&DELEGATED_PDA_ID);
    let committed_state_pda = committed_state_pda_from_pubkey(&DELEGATED_PDA_ID);

    // Save the new state data before undelegating
    let delegated_pda_state_before_undelegation =
        banks.get_account(DELEGATED_PDA_ID).await.unwrap().unwrap();
    let new_state_data_before_finalize = delegated_pda_state_before_undelegation.data.clone();

    // Submit the undelegate tx
    let ix = dlp::instruction::undelegate(
        payer.pubkey(),
        DELEGATED_PDA_ID,
        DELEGATED_PDA_OWNER_ID,
        payer.pubkey(),
    );
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[&payer], blockhash);
    let res = banks.process_transaction(tx).await;
    println!("{:?}", res);
    assert!(res.is_ok());

    // Assert the state_diff was closed
    let new_state_account = banks.get_account(committed_state_pda).await.unwrap();
    assert!(new_state_account.is_none());

    // Assert the delegation_pda was closed
    let delegation_account = banks.get_account(delegation_pda).await.unwrap();
    assert!(delegation_account.is_none());

    // Assert the delegation_pda was closed
    let delegation_account = banks.get_account(delegation_pda).await.unwrap();
    assert!(delegation_account.is_none());

    // Assert the delegated account seeds pda was closed
    let seeds_pda = delegated_account_seeds_pda_from_pubkey(&DELEGATED_PDA_ID);
    let seeds_pda_account = banks.get_account(seeds_pda).await.unwrap();
    assert!(seeds_pda_account.is_none());

    // Assert that the account owner is now set to the owner program
    let pda_account = banks.get_account(DELEGATED_PDA_ID).await.unwrap().unwrap();
    assert!(pda_account.owner.eq(&DELEGATED_PDA_OWNER_ID));

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
        delegation_record_pda_from_pubkey(&DELEGATED_PDA_ID),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: DELEGATION_RECORD_ACCOUNT_DATA.into(),
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the delegated account seeds PDA
    program_test.add_account(
        delegated_account_seeds_pda_from_pubkey(&DELEGATED_PDA_ID),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: DELEGATED_ACCOUNT_SEEDS_PDA.into(),
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
