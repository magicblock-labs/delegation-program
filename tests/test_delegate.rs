use solana_program::pubkey::Pubkey;
use solana_program::{hash::Hash, native_token::LAMPORTS_PER_SOL, system_program};
use solana_program_test::{processor, BanksClient, ProgramTest};
use solana_sdk::{
    account::Account,
    pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

use dlp::consts::BUFFER;

pub const PDA_ID: Pubkey = pubkey!("98WCwJLrk9AZxZpmohpjBamJiUbYw5tQcqH4jWv7xS4S");
pub const PDA_OWNER_ID: Pubkey = pubkey!("wormH7q6y9EBUUL6EyptYhryxs6HoJg8sPK3LMfoNf4");

#[tokio::test]
async fn test_delegate() {
    // Setup
    let (mut banks, payer, _, blockhash) = setup_program_test_env().await;
    // Submit the delegate tx
    let ix = dlp::instruction::delegate(
        payer.pubkey(),
        PDA_ID,
        PDA_OWNER_ID,
        payer.pubkey(),
        system_program::id(),
    );
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[&payer], blockhash);
    let res = banks.process_transaction(tx).await;
    println!("{:?}", res);
    assert!(res.is_ok());
    // Assert the buffer was created and contains a copy of the PDA account
    let buffer_pda = Pubkey::find_program_address(&[BUFFER, &PDA_ID.to_bytes()], &dlp::id());
    let buffer_account = banks.get_account(buffer_pda.0).await.unwrap().unwrap();
    let pda_account = banks.get_account(PDA_ID).await.unwrap().unwrap();
    assert_eq!(buffer_account.data, pda_account.data);
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
        PDA_ID,
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
            owner: PDA_OWNER_ID,
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks, payer, blockhash) = program_test.start().await;
    (banks, payer, payer_alt, blockhash)
}
