use crate::fixtures::{create_program_config_data, TEST_AUTHORITY};
use dlp::pda::{fees_vault_pda, program_config_from_program_id};
use dlp::state::FeesVault;
use solana_program::rent::Rent;
use solana_program::{hash::Hash, native_token::LAMPORTS_PER_SOL, system_program};
use solana_program_test::{BanksClient, BanksClientError, ProgramTest};
use solana_sdk::instruction::InstructionError;
use solana_sdk::transaction::TransactionError;
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
    let (banks, payer, admin, blockhash) = setup_program_test_env().await;

    // Submit the claim fees tx with wrong receiver
    let wrong_receiver = Pubkey::new_unique();
    let ix = dlp::instruction_builder::protocol_claim_fees(wrong_receiver);
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[&payer], blockhash);
    let res = banks.process_transaction(tx).await;

    // Assert that the transaction fails because fees_receiver doesn't match stored value
    assert!(
        matches!(
            res,
            Err(BanksClientError::TransactionError(
                TransactionError::InstructionError(0, InstructionError::InvalidAccountData)
            ))
        ),
        "Expected InvalidAccountData error, got {res:?}",
    );

    // Assert that the fees vault still has the initial lamports
    let fees_vault_pda = fees_vault_pda();
    let vault_after = banks.get_account(fees_vault_pda).await.unwrap().unwrap();
    assert_eq!(vault_after.lamports, LAMPORTS_PER_SOL);

    // Assert that the admin account still has the initial lamports
    let admin_account = banks.get_account(admin.pubkey()).await.unwrap().unwrap();
    assert_eq!(admin_account.lamports, LAMPORTS_PER_SOL);

    // Assert that the fees receiver account still has the initial lamports
    let fees_receiver_account = banks.get_account(wrong_receiver).await.unwrap();
    assert_eq!(fees_receiver_account, None);
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
    assert!(
        matches!(
            res,
            Err(BanksClientError::TransactionError(
                TransactionError::InstructionError(0, InstructionError::InvalidArgument)
            ))
        ),
        "Expected InvalidArgument error, got {res:?}",
    );

    // Assert that the admin account still has the initial lamports
    let admin_account = banks.get_account(admin.pubkey()).await.unwrap().unwrap();
    assert_eq!(admin_account.lamports, LAMPORTS_PER_SOL);

    // Assert that the fees vault still has the initial lamports
    let fees_vault_pda = fees_vault_pda();
    let fees_vault_account = banks.get_account(fees_vault_pda).await.unwrap().unwrap();
    assert_eq!(fees_vault_account.lamports, LAMPORTS_PER_SOL);
}

#[tokio::test]
async fn test_protocol_claim_fees_noop() {
    let (banks, payer, admin, blockhash) = setup_program_test_env().await;
    let fees_vault = fees_vault_pda();

    // First claim: drain to min rent
    let ix = dlp::instruction_builder::protocol_claim_fees(admin.pubkey());
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[&payer], blockhash);
    banks.process_transaction(tx).await.unwrap();

    // Snapshot balances
    let vault_before = banks
        .get_account(fees_vault)
        .await
        .unwrap()
        .unwrap()
        .lamports;
    let admin_before = banks
        .get_account(admin.pubkey())
        .await
        .unwrap()
        .unwrap()
        .lamports;

    // Second claim: should be a no-op and return Ok(())
    let ix = dlp::instruction_builder::protocol_claim_fees(admin.pubkey());
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[&payer], blockhash);
    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok());

    let vault_after = banks
        .get_account(fees_vault)
        .await
        .unwrap()
        .unwrap()
        .lamports;
    let admin_after = banks
        .get_account(admin.pubkey())
        .await
        .unwrap()
        .unwrap()
        .lamports;
    assert_eq!(vault_after, vault_before);
    assert_eq!(admin_after, admin_before);
}

#[tokio::test]
async fn test_protocol_claim_fees_insufficient_funds() {
    // Custom env: vault funded below min rent
    use solana_program_test::ProgramTest;
    use solana_sdk::{account::Account, system_program};
    let mut program_test = ProgramTest::new("dlp", dlp::ID, None);
    program_test.prefer_bpf(true);

    let admin = Keypair::from_bytes(&TEST_AUTHORITY).unwrap();
    program_test.add_account(
        admin.pubkey(),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Prepare FeesVault with correct data but underfunded
    let mut buf = vec![];
    FeesVault {
        fees_receiver: admin.pubkey(),
    }
    .to_bytes_with_discriminator(&mut buf)
    .unwrap();
    let min_rent = Rent::default().minimum_balance(buf.len());
    let underfunded = min_rent.saturating_sub(1);
    program_test.add_account(
        fees_vault_pda(),
        Account {
            lamports: underfunded,
            data: buf,
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Program config (not used by processor here but commonly present)
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

    let ix = dlp::instruction_builder::protocol_claim_fees(admin.pubkey());
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[&payer], blockhash);
    let res = banks.process_transaction(tx).await;
    assert!(
        matches!(
            res,
            Err(BanksClientError::TransactionError(
                TransactionError::InstructionError(0, InstructionError::InsufficientFunds)
            ))
        ),
        "Expected InsufficientFunds error, got {res:?}",
    );
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
