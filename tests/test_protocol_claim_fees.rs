use crate::fixtures::{create_program_config_data, TEST_AUTHORITY};
use dlp::pda::{fees_vault_pda, program_config_from_program_id};
use dlp::state::FeesVault;
use solana_program::rent::Rent;
use solana_program::{hash::Hash, native_token::LAMPORTS_PER_SOL, system_program};
use solana_program_test::{BanksClient, ProgramTest};
use solana_sdk::{
    account::Account,
    pubkey::Pubkey,
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
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[&payer], blockhash);
    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok());

    // Assert that fees vault now only have the rent exemption amount
    let min_rent = Rent::default().minimum_balance(FeesVault::default().size_with_discriminator());
    let fees_vault_account = banks.get_account(fees_vault_pda).await.unwrap();
    assert!(fees_vault_account.is_some());
    assert_eq!(fees_vault_account.unwrap().lamports, min_rent);

    // Assert that the admin account now has the fees
    let admin_account = banks.get_account(admin.pubkey()).await.unwrap();
    assert_eq!(
        admin_account.unwrap().lamports,
        LAMPORTS_PER_SOL * 2 - min_rent
    );

    // Verify FeesVault still stores the admin as receiver
    let data = banks
        .get_account(fees_vault_pda)
        .await
        .unwrap()
        .unwrap()
        .data;
    let vault = FeesVault::try_from_bytes_with_discriminator(&data).unwrap();
    assert_eq!(vault.fees_receiver, admin.pubkey());
}

#[tokio::test]
async fn test_protocol_claim_fees_wrong_receiver() {
    // Setup
    let (banks, payer, _admin, blockhash) = setup_program_test_env().await;

    // Submit the claim fees tx with wrong receiver
    let wrong_receiver = Pubkey::new_unique();
    let ix = dlp::instruction_builder::protocol_claim_fees(wrong_receiver);
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[&payer], blockhash);
    let res = banks.process_transaction(tx).await;

    // Assert that the transaction fails because fees_receiver doesn't match stored value
    assert!(res.is_err());
}

#[tokio::test]
async fn test_protocol_claim_fees_self() {
    // Setup
    let (banks, payer, admin, blockhash) = setup_program_test_env().await;

    // Set fees receiver to fees vault
    let fees_receiver = fees_vault_pda();
    let ix = dlp::instruction_builder::set_fees_receiver(admin.pubkey(), fees_receiver);
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &admin],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok());

    let ix = dlp::instruction_builder::protocol_claim_fees(fees_receiver);
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[&payer], blockhash);
    let res = banks.process_transaction(tx).await;

    // Assert that the transaction fails because fees_receiver is the same as the fees vault
    assert!(res.is_err());
}

async fn setup_program_test_env() -> (BanksClient, Keypair, Keypair, Hash) {
    let mut program_test = ProgramTest::new("dlp", dlp::ID, None);
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
    let mut buffer = vec![];
    FeesVault {
        fees_receiver: admin_keypair.pubkey(),
    }
    .to_bytes_with_discriminator(&mut buffer)
    .unwrap();
    program_test.add_account(
        fees_vault_pda(),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: buffer,
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the fees program config account
    program_test.add_account(
        program_config_from_program_id(&dlp::ID),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: create_program_config_data(Pubkey::new_unique()),
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks, payer, blockhash) = program_test.start().await;
    (banks, payer, admin_keypair, blockhash)
}
