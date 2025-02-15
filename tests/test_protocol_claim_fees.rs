use crate::fixtures::TEST_AUTHORITY;
use dlp::pda::fees_vault_pda;
use solana_program::rent::Rent;
use solana_program::{hash::Hash, native_token::LAMPORTS_PER_SOL, system_program};
use solana_program_test::{processor, BanksClient, ProgramTest};
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

mod fixtures;

#[tokio::test]
async fn test_protocol_claim_fees() {
    // Setup
    let (banks, payer, admin, blockhash) = setup_program_test_env().await;

    let fees_vault_pda = fees_vault_pda();

    // Submit the claim fees tx
    let ix = dlp::instruction_builder::protocol_claim_fees(admin.pubkey());
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &admin],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok());

    // Assert that fees vault now only have the rent exemption amount
    let fees_vault_account = banks.get_account(fees_vault_pda).await.unwrap();
    assert!(fees_vault_account.is_some());
    assert_eq!(
        fees_vault_account.unwrap().lamports,
        Rent::default().minimum_balance(8)
    );

    // Assert that the admin account now has the fees
    let admin_account = banks.get_account(admin.pubkey()).await.unwrap();
    assert_eq!(
        admin_account.unwrap().lamports,
        LAMPORTS_PER_SOL * 2 - Rent::default().minimum_balance(8)
    );
}

async fn setup_program_test_env() -> (BanksClient, Keypair, Keypair, Hash) {
    let mut program_test = ProgramTest::new("dlp", dlp::ID, processor!(dlp::process_instruction));
    program_test.prefer_bpf(true);

    let admin_keypair = Keypair::from_bytes(&TEST_AUTHORITY).unwrap();

    program_test.add_account(
        admin_keypair.pubkey(),
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
        fees_vault_pda(),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks, payer, blockhash) = program_test.start().await;
    (banks, payer, admin_keypair, blockhash)
}
