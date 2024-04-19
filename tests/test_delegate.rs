use solana_program::{hash::Hash, native_token::LAMPORTS_PER_SOL, rent::Rent, system_program};
use solana_program::pubkey::Pubkey;
use solana_program_test::{processor, read_file, BanksClient, ProgramTest};
use solana_sdk::{account::Account, pubkey, signature::{Keypair, Signer}, transaction::Transaction};

pub const PDA_ID: Pubkey = pubkey!("98WCwJLrk9AZxZpmohpjBamJiUbYw5tQcqH4jWv7xS4S");
pub const PDA_OWNER_ID: Pubkey = pubkey!("wormH7q6y9EBUUL6EyptYhryxs6HoJg8sPK3LMfoNf4");

#[tokio::test]
async fn test_delegate() {
    // Setup
    let (mut banks, payer, _, blockhash) = setup_program_test_env().await;
    // Submit tx
    let ix = dlp::instruction::delegate(payer.pubkey(), PDA_ID, PDA_OWNER_ID, payer.pubkey(), system_program::id());
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[&payer], blockhash);
    let res = banks.process_transaction(tx).await;
    println!("{:?}", res);
    assert!(res.is_ok());
}

async fn setup_program_test_env() -> (BanksClient, Keypair, Keypair, Hash) {
    let mut program_test = ProgramTest::new("dlp", dlp::ID, processor!(dlp::process_instruction));
    program_test.prefer_bpf(true);

    // Setup metadata program
    let data = read_file(&"tests/buffers/metadata_program.bpf");
    program_test.add_account(
        mpl_token_metadata::ID,
        Account {
            lamports: Rent::default().minimum_balance(data.len()).max(1),
            data,
            owner: solana_sdk::bpf_loader::id(),
            executable: true,
            rent_epoch: 0,
        },
    );

    // Setup alt payer
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
    let payer_alt = Keypair::new();
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
