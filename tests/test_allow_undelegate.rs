use solana_program::rent::Rent;
use solana_program::{hash::Hash, native_token::LAMPORTS_PER_SOL, system_program};
use solana_program_test::{processor, read_file, BanksClient, ProgramTest};
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

use dlp::pda::{
    committed_state_pda_from_pubkey, committed_state_record_pda_from_pubkey,
    delegation_metadata_pda_from_pubkey, delegation_record_pda_from_pubkey,
};

use crate::fixtures::{
    COMMIT_NEW_STATE_ACCOUNT_DATA, COMMIT_STATE_RECORD_ACCOUNT_DATA, DELEGATED_PDA,
    DELEGATED_PDA_ID, DELEGATED_PDA_OWNER_ID, DELEGATION_METADATA_PDA,
    DELEGATION_RECORD_ACCOUNT_DATA, EXTERNAL_ALLOW_UNDELEGATION_INSTRUCTION_DISCRIMINATOR,
};

mod fixtures;

#[tokio::test]
async fn test_allow_undelegate() {
    // Setup
    let (mut banks, payer, _, blockhash) = setup_program_test_env().await;

    // // Assert the delegated account seeds pda was closed
    // let seeds_pda = delegation_metadata_pda_from_pubkey(&DELEGATED_PDA_ID);
    // let seeds_pda_account = banks.get_account(seeds_pda).await.unwrap();
    // assert!(seeds_pda_account.is_none());

    // Submit the allow undelegation tx
    let ix = dlp::instruction::allow_undelegate(
        DELEGATED_PDA_ID,
        DELEGATED_PDA_OWNER_ID,
        EXTERNAL_ALLOW_UNDELEGATION_INSTRUCTION_DISCRIMINATOR.to_vec(),
    );
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[&payer], blockhash);
    let res = banks.process_transaction(tx).await;
    println!("{:?}", res);
    assert!(res.is_ok());
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
            data: DELEGATED_PDA.into(),
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
        delegation_metadata_pda_from_pubkey(&DELEGATED_PDA_ID),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: DELEGATION_METADATA_PDA.into(),
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the committed state PDA
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
