use solana_program::{hash::Hash, native_token::LAMPORTS_PER_SOL, system_program};
use solana_program_test::{processor, BanksClient, ProgramTest};
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

use dlp::pda::{
    delegation_metadata_pda_from_pubkey, delegation_record_pda_from_pubkey,
    validator_fees_vault_pda_from_pubkey,
};

use crate::fixtures::{
    get_delegation_metadata_data_on_curve, get_delegation_record_on_curve_data, ON_CURVE_KEYPAIR,
    TEST_AUTHORITY,
};

mod fixtures;

#[tokio::test]
async fn test_undelegate_on_curve() {
    // Setup
    let (mut banks, validator, delegated_on_curve, blockhash) = setup_program_test_env().await;

    // Retrieve the accounts
    let delegation_pda = delegation_record_pda_from_pubkey(&delegated_on_curve.pubkey());

    // Submit the undelegate tx
    let ix = dlp::instruction::undelegate(
        validator.pubkey(),
        delegated_on_curve.pubkey(),
        system_program::id(),
        validator.pubkey(),
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

    // Assert the delegation_pda was closed
    let delegation_account = banks.get_account(delegation_pda).await.unwrap();
    assert!(delegation_account.is_none());

    // Assert the delegated metadata account pda was closed
    let seeds_pda = delegation_metadata_pda_from_pubkey(&delegated_on_curve.pubkey());
    let seeds_pda_account = banks.get_account(seeds_pda).await.unwrap();
    assert!(seeds_pda_account.is_none());

    // Assert that the account owner is now set to the system program
    let pda_account = banks
        .get_account(delegated_on_curve.pubkey())
        .await
        .unwrap()
        .unwrap();
    assert!(pda_account.owner.eq(&system_program::id()));
}

async fn setup_program_test_env() -> (BanksClient, Keypair, Keypair, Hash) {
    let mut program_test = ProgramTest::new("dlp", dlp::ID, processor!(dlp::process_instruction));
    program_test.prefer_bpf(true);
    let validator = Keypair::from_bytes(&TEST_AUTHORITY).unwrap();
    let payer_alt = Keypair::from_bytes(&ON_CURVE_KEYPAIR).unwrap();

    // Setup a delegated on curve account
    program_test.add_account(
        payer_alt.pubkey(),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the delegated record PDA
    let data = get_delegation_record_on_curve_data(payer_alt.pubkey());
    program_test.add_account(
        delegation_record_pda_from_pubkey(&payer_alt.pubkey()),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data,
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the delegated account metadata PDA
    let data = get_delegation_metadata_data_on_curve(Some(LAMPORTS_PER_SOL), Some(true));
    program_test.add_account(
        delegation_metadata_pda_from_pubkey(&payer_alt.pubkey()),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data,
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the validator keypair
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

    // Setup the validator fees vault
    program_test.add_account(
        validator_fees_vault_pda_from_pubkey(&validator.pubkey()),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks, _, blockhash) = program_test.start().await;
    (banks, validator, payer_alt, blockhash)
}
