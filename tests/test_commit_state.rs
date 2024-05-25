use solana_program::pubkey::Pubkey;
use solana_program::{hash::Hash, native_token::LAMPORTS_PER_SOL};
use solana_program_test::{processor, BanksClient, ProgramTest};
use solana_sdk::{
    account::Account,
    pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

use dlp::pda::{
    committed_state_pda_from_pubkey, committed_state_record_pda_from_pubkey,
    delegation_record_pda_from_pubkey,
};
use dlp::state::CommitRecord;
use dlp::utils_account::AccountDeserialize;

pub const DELEGATED_PDA_ID: Pubkey = pubkey!("8k2V7EzQtNg38Gi9HK5ZtQYp1YpGKNGrMcuGa737gZX4");

#[tokio::test]
async fn test_commit_new_state() {
    // Setup
    let (mut banks, payer, _, blockhash) = setup_program_test_env().await;
    let new_state = vec![0, 1, 2, 9, 9, 9, 6, 7, 8, 9];

    // Commit the state for the delegated account
    let ix = dlp::instruction::commit_state(payer.pubkey(), DELEGATED_PDA_ID, new_state.clone());
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[&payer], blockhash);
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
    assert_eq!(state_commit_record.identity, payer.pubkey());
    assert!(state_commit_record.timestamp > 0);
}

async fn setup_program_test_env() -> (BanksClient, Keypair, Keypair, Hash) {
    let mut program_test = ProgramTest::new("dlp", dlp::ID, processor!(dlp::process_instruction));
    program_test.prefer_bpf(true);
    let payer_alt = Keypair::new();

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
            data: vec![
                100, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 43, 85, 175, 207, 195, 148, 154, 129, 218,
                62, 110, 177, 81, 112, 72, 172, 141, 157, 3, 211, 24, 26, 191, 79, 101, 191, 48,
                19, 105, 181, 70, 132, 0, 0, 0, 0, 0, 0, 0, 0, 224, 147, 4, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0,
            ],
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks, payer, blockhash) = program_test.start().await;
    (banks, payer, payer_alt, blockhash)
}
