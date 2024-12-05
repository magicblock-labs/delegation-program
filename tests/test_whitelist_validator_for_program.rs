use crate::fixtures::{DELEGATED_PDA_OWNER_ID, TEST_AUTHORITY};
use borsh::BorshDeserialize;
use dlp::pda::program_config_pda_from_pubkey;
use dlp::state::ProgramConfig;
use solana_program::rent::Rent;
use solana_program::{hash::Hash, native_token::LAMPORTS_PER_SOL, system_program};
use solana_program_test::{processor, read_file, BanksClient, ProgramTest};
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

mod fixtures;

#[tokio::test]
async fn test_whitelist_validator_for_program() {
    // Setup
    let (mut banks, _, validator, blockhash) = setup_program_test_env().await;

    let ix = dlp::instruction_builder::whitelist_validator_for_program(
        validator.pubkey(),
        validator.pubkey(),
        DELEGATED_PDA_OWNER_ID,
        true,
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&validator.pubkey()),
        &[&validator],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    println!("{:?}", res);
    assert!(res.is_ok());

    // Check that the validator is whitelisted
    let validator_whitelist_account = banks
        .get_account(program_config_pda_from_pubkey(&DELEGATED_PDA_OWNER_ID))
        .await;
    let whitelist =
        ProgramConfig::try_from_slice(&validator_whitelist_account.unwrap().unwrap().data)
            .unwrap();
    assert!(whitelist.approved_validators.contains(&validator.pubkey()));
}

#[tokio::test]
async fn test_remove_validator_for_program() {
    // Setup
    let (mut banks, _, validator, blockhash) = setup_program_test_env().await;

    let ix = dlp::instruction_builder::whitelist_validator_for_program(
        validator.pubkey(),
        validator.pubkey(),
        DELEGATED_PDA_OWNER_ID,
        true,
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&validator.pubkey()),
        &[&validator],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    println!("{:?}", res);
    assert!(res.is_ok());

    // Remove the validator
    let ix = dlp::instruction_builder::whitelist_validator_for_program(
        validator.pubkey(),
        validator.pubkey(),
        DELEGATED_PDA_OWNER_ID,
        false,
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&validator.pubkey()),
        &[&validator],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    println!("{:?}", res);
    assert!(res.is_ok());

    // Check that the validator is not whitelisted
    let validator_whitelist_account = banks
        .get_account(program_config_pda_from_pubkey(&DELEGATED_PDA_OWNER_ID))
        .await;
    let whitelist =
        ProgramConfig::try_from_slice(&validator_whitelist_account.unwrap().unwrap().data)
            .unwrap();
    assert!(!whitelist.approved_validators.contains(&validator.pubkey()));
}

async fn setup_program_test_env() -> (BanksClient, Keypair, Keypair, Hash) {
    let mut program_test = ProgramTest::new("dlp", dlp::ID, processor!(dlp::process_instruction));
    program_test.prefer_bpf(true);
    let validator = Keypair::from_bytes(&TEST_AUTHORITY).unwrap();

    program_test.add_account(
        validator.pubkey(),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup program to test undelegation
    let data = read_file("tests/buffers/test_delegation.so");
    program_test.add_account(
        DELEGATED_PDA_OWNER_ID,
        Account {
            lamports: Rent::default().minimum_balance(data.len()),
            data,
            owner: solana_sdk::bpf_loader::id(),
            executable: true,
            rent_epoch: 0,
        },
    );

    let (banks, payer, blockhash) = program_test.start().await;
    (banks, payer, validator, blockhash)
}
