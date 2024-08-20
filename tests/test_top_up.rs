use solana_program::{hash::Hash, native_token::LAMPORTS_PER_SOL, system_program};
use solana_program::pubkey::Pubkey;
use solana_program_test::{BanksClient, processor, ProgramTest};
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use dlp::consts::FEES_VAULT;

mod fixtures;

#[tokio::test]
async fn test_undelegate() {
    // Setup
    let (mut banks, payer, _, blockhash) = setup_program_test_env().await;


    // Submit the undelegate tx
    let ix = dlp::instruction::top_up_ephemeral_balance(
        payer.pubkey(),
        100000,
    );
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[&payer], blockhash);
    let res = banks.process_transaction(tx).await;
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

    // Setup the fees vault account
    program_test.add_account(
        Pubkey::find_program_address(&[FEES_VAULT], &dlp::id()).0,
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
