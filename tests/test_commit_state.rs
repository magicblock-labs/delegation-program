use borsh::BorshDeserialize;
use solana_program::{hash::Hash, native_token::LAMPORTS_PER_SOL, system_program};
use solana_program_test::{processor, BanksClient, ProgramTest};
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

use dlp::instruction::CommitAccountArgs;
use dlp::pda::{
    committed_state_pda_from_pubkey, committed_state_record_pda_from_pubkey,
    delegation_metadata_pda_from_pubkey, delegation_record_pda_from_pubkey,
};
use dlp::state::{CommitRecord, DelegationMetadata};
use dlp::utils_account::AccountDeserialize;

use crate::fixtures::{
    DELEGATED_PDA_ID, DELEGATION_METADATA_PDA, DELEGATION_RECORD_ACCOUNT_DATA, TEST_AUTHORITY,
};

mod fixtures;

#[tokio::test]
async fn test_commit_new_state() {
    // Setup
    let (mut banks, _, authority, blockhash) = setup_program_test_env().await;
    let new_state = vec![0, 1, 2, 9, 9, 9, 6, 7, 8, 9];

    let commit_args = CommitAccountArgs {
        data: new_state.clone(),
        slot: 100,
        allow_undelegation: true,
    };

    // Commit the state for the delegated account
    let ix = dlp::instruction::commit_state(authority.pubkey(), DELEGATED_PDA_ID, commit_args);
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&authority.pubkey()),
        &[&authority],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    println!("{:?}", res);
    assert!(res.is_ok());

    // Assert the state commitment was created and contains the new state
    let committed_state_pda = committed_state_pda_from_pubkey(&DELEGATED_PDA_ID);
    let new_state_account = banks
        .get_account(committed_state_pda)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(new_state_account.data, new_state.clone());

    // Assert the record about the commitment exists
    let state_commit_record_pda = committed_state_record_pda_from_pubkey(&DELEGATED_PDA_ID);
    let state_commit_record_account = banks
        .get_account(state_commit_record_pda)
        .await
        .unwrap()
        .unwrap();
    let state_commit_record =
        CommitRecord::try_from_bytes(&state_commit_record_account.data).unwrap();
    assert_eq!(state_commit_record.account, DELEGATED_PDA_ID);
    assert_eq!(state_commit_record.identity, authority.pubkey());
    assert_eq!(state_commit_record.slot, 100);

    let delegation_metadata_pda = delegation_metadata_pda_from_pubkey(&DELEGATED_PDA_ID);
    let delegation_metadata_account = banks
        .get_account(delegation_metadata_pda)
        .await
        .unwrap()
        .unwrap();
    let delegation_metadata =
        DelegationMetadata::try_from_slice(&delegation_metadata_account.data).unwrap();
    assert_eq!(delegation_metadata.is_undelegatable, true);
}

async fn setup_program_test_env() -> (BanksClient, Keypair, Keypair, Hash) {
    let mut program_test = ProgramTest::new("dlp", dlp::ID, processor!(dlp::process_instruction));
    program_test.prefer_bpf(true);

    let payer_alt = Keypair::from_bytes(&TEST_AUTHORITY).unwrap();

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

    let (banks, payer, blockhash) = program_test.start().await;
    (banks, payer, payer_alt, blockhash)
}
