use solana_program::pubkey::Pubkey;
use solana_program::rent::Rent;
use solana_program::{hash::Hash, native_token::LAMPORTS_PER_SOL, system_program};
use solana_program_test::{processor, read_file, BanksClient, ProgramTest};
use solana_sdk::{
    account::Account,
    pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

use dlp::consts::{BUFFER, COMMIT_RECORD, DELEGATION, STATE_DIFF};

pub const DELEGATED_PDA_ID: Pubkey = pubkey!("8k2V7EzQtNg38Gi9HK5ZtQYp1YpGKNGrMcuGa737gZX4");
pub const DELEGATED_PDA_OWNER_ID: Pubkey = pubkey!("3vAK9JQiDsKoQNwmcfeEng4Cnv22pYuj1ASfso7U4ukF");

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

    // Submit the undelegate tx
    let ix = dlp::instruction::undelegate(
        payer.pubkey(),
        DELEGATED_PDA_ID,
        DELEGATED_PDA_OWNER_ID,
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

    // Setup the commit state PDA
    program_test.add_account(
        Pubkey::find_program_address(&[STATE_DIFF, &DELEGATED_PDA_ID.to_bytes()], &dlp::id()).0,
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 11],
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
            data: vec![
                101, 0, 0, 0, 0, 0, 0, 0, 202, 37, 188, 175, 199, 216, 218, 84, 43, 75, 255, 157,
                215, 202, 195, 114, 139, 194, 225, 131, 177, 111, 103, 238, 162, 225, 196, 178, 29,
                219, 96, 127, 115, 7, 118, 65, 61, 170, 109, 216, 57, 214, 57, 150, 28, 32, 145,
                234, 70, 215, 243, 242, 145, 103, 150, 11, 142, 149, 177, 109, 222, 157, 148, 7,
                97, 218, 60, 102, 0, 0, 0, 0,
            ],
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
