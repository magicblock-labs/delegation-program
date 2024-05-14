use solana_program::pubkey::Pubkey;
use solana_program::rent::Rent;
use solana_program::{hash::Hash, native_token::LAMPORTS_PER_SOL, system_program};
use solana_program_test::{processor, read_file, BanksClient, ProgramTest};
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

use dlp::consts::BUFFER;

use crate::fixtures::{
    DELEGATED_PDA_ID, DELEGATED_PDA_OWNER_ID, EXTERNAL_DELEGATE_INSTRUCTION_DISCRIMINATOR,
};

mod fixtures;

#[tokio::test]
async fn test_delegate() {
    // Setup
    let (mut banks, payer, _, blockhash) = setup_program_test_env().await;

    // Save the PDA before delegation
    let pda_before_delegation = banks.get_account(DELEGATED_PDA_ID).await.unwrap().unwrap();
    let pda_data_before_delegation = pda_before_delegation.data.clone();

    // Submit the delegate tx
    let ix = dlp::instruction::delegate(
        payer.pubkey(),
        DELEGATED_PDA_ID,
        system_program::id(),
        dlp::id(),
        DELEGATED_PDA_OWNER_ID,
        EXTERNAL_DELEGATE_INSTRUCTION_DISCRIMINATOR.to_vec(),
    );

    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[&payer], blockhash);
    let res = banks.process_transaction(tx).await;

    println!("{:?}", res);
    assert!(res.is_ok());

    // Assert the buffer was closed
    let buffer_pda = Pubkey::find_program_address(
        &[BUFFER, &DELEGATED_PDA_ID.to_bytes()],
        &DELEGATED_PDA_OWNER_ID,
    );
    let buffer_account = banks.get_account(buffer_pda.0).await.unwrap();
    assert!(buffer_account.is_none());

    // Assert the PDA was delegated => owner is set to the delegation program
    let pda_account = banks.get_account(DELEGATED_PDA_ID).await.unwrap().unwrap();
    assert!(pda_account.owner.eq(&dlp::id()));

    // Assert the PDA data was not changed
    assert_eq!(pda_data_before_delegation, pda_account.data);
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

    // Setup a PDA
    program_test.add_account(
        DELEGATED_PDA_ID,
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
            owner: DELEGATED_PDA_OWNER_ID,
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup program to test delegation
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
