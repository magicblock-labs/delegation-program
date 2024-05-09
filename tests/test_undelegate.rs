use solana_program::pubkey::Pubkey;
use solana_program::{hash::Hash, native_token::LAMPORTS_PER_SOL, system_program};
use solana_program_test::{processor, BanksClient, ProgramTest};
use solana_sdk::{
    account::Account,
    pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

use dlp::consts::{BUFFER, COMMIT_RECORD, DELEGATION, STATE_DIFF};

pub const DELEGATED_PDA_ID: Pubkey = pubkey!("DSpv6esFqXfNsiem9RNyXEagGgLY18i2RdepkSbQn86H");
pub const DELEGATED_PDA_OWNER_ID: Pubkey = pubkey!("99B2bTijsU6f1GCT73HmdR7HCFFjGMBcPZY6jZ96ynrR");

#[tokio::test]
async fn test_undelegate() {
    // Setup
    let (mut banks, payer, _, blockhash) = setup_program_test_env().await;

    // Retrieve the accounts
    let buffer = Pubkey::find_program_address(&[BUFFER, &DELEGATED_PDA_ID.to_bytes()], &dlp::id());
    let delegation_pda =
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

    // Submit the delegate tx
    let ix = dlp::instruction::delegate(
        payer.pubkey(),
        DELEGATED_PDA_ID,
        dlp::id(),
        payer.pubkey(),
        system_program::id(),
    );
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[&payer], blockhash);
    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok());

    // Submit the commit tx
    let new_state = vec![0, 1, 2, 9, 9, 9, 6, 7, 8, 9];
    let ix = dlp::instruction::commit_state(
        payer.pubkey(),
        DELEGATED_PDA_ID,
        0,
        system_program::id(),
        new_state.clone(),
    );
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[&payer], blockhash);
    let res = banks.process_transaction(tx).await;
    println!("{:?}", res);
    assert!(res.is_ok());

    // Submit the undelegate tx
    let ix = dlp::instruction::undelegate(
        payer.pubkey(),
        DELEGATED_PDA_ID,
        dlp::id(),
        buffer.0,
        new_state_pda.0,
        commit_state_record_pda.0,
        delegation_pda.0,
        payer.pubkey(),
    );
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[&payer], blockhash);
    let res = banks.process_transaction(tx).await;
    println!("{:?}", res);
    assert!(res.is_ok());

    // Assert the state_diff was closed
    let new_state_account = banks.get_account(new_state_pda.0).await.unwrap();
    assert!(new_state_account.is_none());

    // Assert the delegation_pda was closed
    let delegation_account = banks.get_account(delegation_pda.0).await.unwrap();
    assert!(delegation_account.is_none());

    // Assert the commit_state_record_pda was closed
    let commit_state_record_account = banks.get_account(commit_state_record_pda.0).await.unwrap();
    assert!(commit_state_record_account.is_none());
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

    let (banks, payer, blockhash) = program_test.start().await;
    (banks, payer, payer_alt, blockhash)
}
