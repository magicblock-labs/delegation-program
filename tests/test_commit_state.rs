use solana_program::pubkey::Pubkey;
use solana_program::{hash::Hash, native_token::LAMPORTS_PER_SOL, system_program};
use solana_program_test::{processor, BanksClient, ProgramTest};
use solana_sdk::{
    account::Account,
    pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

use dlp::consts::{COMMIT_RECORD, DELEGATION, STATE_DIFF};
use dlp::state::CommitState;
use dlp::utils::AccountDeserialize;

pub const DELEGATED_PDA_ID: Pubkey = pubkey!("8k2V7EzQtNg38Gi9HK5ZtQYp1YpGKNGrMcuGa737gZX4");

#[tokio::test]
async fn test_commit_new_state() {
    // Setup
    let (mut banks, payer, _, blockhash) = setup_program_test_env().await;
    let new_state = vec![0, 1, 2, 9, 9, 9, 6, 7, 8, 9];

    // Commit the state for the delegated account
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

    // Assert the state commitment was created and contains the new state
    let new_state_pda =
        Pubkey::find_program_address(&[STATE_DIFF, &DELEGATED_PDA_ID.to_bytes()], &dlp::id());
    let new_state_account = banks.get_account(new_state_pda.0).await.unwrap().unwrap();
    assert_eq!(new_state_account.data, new_state.clone());

    // Assert the record about the commitment exists
    let state_commit_record_pda = Pubkey::find_program_address(
        &[
            COMMIT_RECORD,
            &0u64.to_be_bytes(),
            &DELEGATED_PDA_ID.to_bytes(),
        ],
        &dlp::id(),
    );
    let state_commit_record_account = banks
        .get_account(state_commit_record_pda.0)
        .await
        .unwrap()
        .unwrap();
    let state_commit_record =
        CommitState::try_from_bytes(&state_commit_record_account.data).unwrap();
    assert_eq!(state_commit_record.account, DELEGATED_PDA_ID);
    assert_eq!(state_commit_record.identity, payer.pubkey());
    assert!(state_commit_record.timestamp > 0);

    // Assert the delegation record commits counter is set to 1
    let delegation_pda = Pubkey::find_program_address(
        &[dlp::consts::DELEGATION, &DELEGATED_PDA_ID.to_bytes()],
        &dlp::id(),
    );
    let delegation_account = banks.get_account(delegation_pda.0).await.unwrap().unwrap();
    let delegation_record =
        dlp::state::Delegation::try_from_bytes(&delegation_account.data).unwrap();
    assert_eq!(delegation_record.commits, 1);
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
        Pubkey::find_program_address(&[DELEGATION, &DELEGATED_PDA_ID.to_bytes()], &dlp::id()).0,
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![
                100, 0, 0, 0, 0, 0, 0, 0, 202, 37, 188, 175, 199, 216, 218, 84, 43, 75, 255, 157,
                215, 202, 195, 114, 139, 194, 225, 131, 177, 111, 103, 238, 162, 225, 196, 178, 29,
                219, 96, 127, 43, 85, 175, 207, 195, 148, 154, 129, 218, 62, 110, 177, 81, 112, 72,
                172, 141, 157, 3, 211, 24, 26, 191, 79, 101, 191, 48, 19, 105, 181, 70, 132, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ],
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks, payer, blockhash) = program_test.start().await;
    (banks, payer, payer_alt, blockhash)
}
