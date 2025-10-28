use std::collections::BTreeSet;
use std::vec;

use crate::fixtures::{create_program_config_data, TEST_AUTHORITY};
use borsh::{BorshDeserialize, BorshSerialize};
use dlp::pda::{fees_vault_pda, program_config_from_program_id};
use dlp::state::discriminator::{AccountDiscriminator, AccountWithDiscriminator};
use dlp::state::FeesVault;
use dlp::{impl_to_bytes_with_discriminator_borsh, impl_try_from_bytes_with_discriminator_borsh};
use solana_program::rent::Rent;
use solana_program::{hash::Hash, native_token::LAMPORTS_PER_SOL, system_program};
use solana_program_test::{BanksClient, ProgramTest};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

mod fixtures;

#[derive(BorshSerialize, BorshDeserialize)]
struct OldProgramConfig {
    approved_validators: BTreeSet<Pubkey>,
}

impl AccountWithDiscriminator for OldProgramConfig {
    fn discriminator() -> AccountDiscriminator {
        AccountDiscriminator::ProgramConfig
    }
}

impl_to_bytes_with_discriminator_borsh!(OldProgramConfig);
impl_try_from_bytes_with_discriminator_borsh!(OldProgramConfig);

#[tokio::test]
async fn test_set_fees_receiver() {
    // Setup
    let (banks, payer, admin, blockhash) = setup_program_test_env(false).await;

    let fees_vault_pda = fees_vault_pda();

    let fees_receiver = Pubkey::new_unique();

    // Set the fees receiver to a new account
    let ix = dlp::instruction_builder::set_fees_receiver(admin.pubkey(), fees_receiver);
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &admin],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok());

    // Try claiming to the wrong fees receiver
    let ix = dlp::instruction_builder::protocol_claim_fees(admin.pubkey(), dlp::ID);
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[&payer], blockhash);
    let res = banks.process_transaction(tx).await;
    assert!(res.is_err());

    // Claim to the correct fees receiver
    let ix = dlp::instruction_builder::protocol_claim_fees(fees_receiver, dlp::ID);
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[&payer], blockhash);
    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok());

    // Assert that fees vault now only have the rent exemption amount
    let min_rent = Rent::default().minimum_balance(FeesVault::default().size_with_discriminator());
    let fees_vault_account = banks.get_account(fees_vault_pda).await.unwrap();
    assert!(fees_vault_account.is_some());
    assert_eq!(fees_vault_account.unwrap().lamports, min_rent);

    // Assert that the fees receiver account now has the fees
    let fees_receiver_account = banks.get_account(fees_receiver).await.unwrap();
    assert_eq!(
        fees_receiver_account.unwrap().lamports,
        LAMPORTS_PER_SOL - min_rent
    );

    // Assert that FeesVault deserializes correctly and stores the right fees_receiver
    let data = banks
        .get_account(fees_vault_pda)
        .await
        .unwrap()
        .unwrap()
        .data;
    let vault = FeesVault::try_from_bytes_with_discriminator(&data).unwrap();
    assert_eq!(Pubkey::from(vault.fees_receiver), fees_receiver);
}

#[tokio::test]
async fn test_set_fees_receiver_migration() {
    // Setup
    let (banks, payer, admin, blockhash) = setup_program_test_env_old_fees_vault(true).await;

    let fees_vault_pda = fees_vault_pda();

    let fees_receiver = Pubkey::new_unique();

    // Set the fees receiver to a new account
    let ix = dlp::instruction_builder::set_fees_receiver(admin.pubkey(), fees_receiver);
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &admin],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok());

    // Try claiming to the wrong fees receiver
    let ix = dlp::instruction_builder::protocol_claim_fees(admin.pubkey(), dlp::ID);
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[&payer], blockhash);
    let res = banks.process_transaction(tx).await;
    assert!(res.is_err());

    // Claim to the correct fees receiver
    let ix = dlp::instruction_builder::protocol_claim_fees(fees_receiver, dlp::ID);
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[&payer], blockhash);
    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok());

    // Assert that fees vault now only have the rent exemption amount
    let min_rent = Rent::default().minimum_balance(FeesVault::default().size_with_discriminator());
    let fees_vault_account = banks.get_account(fees_vault_pda).await.unwrap();
    assert!(fees_vault_account.is_some());
    assert_eq!(fees_vault_account.unwrap().lamports, min_rent);

    // Assert that the fees receiver account now has the fees
    let fees_receiver_account = banks.get_account(fees_receiver).await.unwrap();
    assert_eq!(
        fees_receiver_account.unwrap().lamports,
        LAMPORTS_PER_SOL - min_rent
    );

    // Assert that FeesVault deserializes correctly and stores the right fees_receiver
    let data = banks
        .get_account(fees_vault_pda)
        .await
        .unwrap()
        .unwrap()
        .data;
    let vault = FeesVault::try_from_bytes_with_discriminator(&data).unwrap();
    assert_eq!(Pubkey::from(vault.fees_receiver), fees_receiver);
}

async fn setup_program_test_env(migrate: bool) -> (BanksClient, Keypair, Keypair, Hash) {
    // Setup the fees vault account
    let mut buffer = vec![];
    FeesVault {
        fees_receiver: Pubkey::new_unique(),
    }
    .to_bytes_with_discriminator(&mut buffer)
    .unwrap();
    base_setup_program_test_env(migrate, buffer).await
}

async fn setup_program_test_env_old_fees_vault(
    migrate: bool,
) -> (BanksClient, Keypair, Keypair, Hash) {
    base_setup_program_test_env(migrate, vec![]).await
}

async fn base_setup_program_test_env(
    migrate: bool,
    fees_vault_data: Vec<u8>,
) -> (BanksClient, Keypair, Keypair, Hash) {
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
    program_test.add_account(
        fees_vault_pda(),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: fees_vault_data,
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the fees program config account
    let data = if migrate {
        let mut program_config = OldProgramConfig {
            approved_validators: BTreeSet::new(),
        };
        program_config
            .approved_validators
            .insert(Pubkey::new_unique());
        let mut bytes = vec![];
        program_config
            .to_bytes_with_discriminator(&mut bytes)
            .unwrap();
        bytes
    } else {
        create_program_config_data(Pubkey::new_unique())
    };
    program_test.add_account(
        program_config_from_program_id(&dlp::ID),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data,
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks, payer, blockhash) = program_test.start().await;
    (banks, payer, admin_keypair, blockhash)
}
