use borsh::BorshDeserialize;
use dlp::args::CommitStateArgs;
use dlp::pda::{
    commit_record_pda_from_pubkey, commit_state_pda_from_pubkey,
    delegation_metadata_pda_from_pubkey, delegation_record_pda_from_pubkey,
    validator_fees_vault_pda_from_pubkey,
};
use dlp::state::account_try_from_bytes::TryFromBytes;
use dlp::state::{CommitRecord, DelegationMetadata};
use solana_program::rent::Rent;
use solana_program::{hash::Hash, native_token::LAMPORTS_PER_SOL, system_program};
use solana_program_test::{processor, BanksClient, ProgramTest};
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

use crate::fixtures::{
    get_delegation_metadata_data_on_curve, get_delegation_record_on_curve_data, ON_CURVE_KEYPAIR,
    TEST_AUTHORITY,
};

mod fixtures;

#[tokio::test]
async fn test_commit_on_curve() {
    // Setup
    let (mut banks, payer_delegated, validator, blockhash) = setup_program_test_env().await;

    let new_account_balance = 1_000_000;
    let commit_args = CommitStateArgs {
        data: vec![],
        slot: 100,
        allow_undelegation: true,
        lamports: new_account_balance,
    };

    // Commit the state for the delegated account
    let ix = dlp::instruction_builder::commit_state(
        validator.pubkey(),
        payer_delegated.pubkey(),
        system_program::ID,
        commit_args,
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

    // Assert the state commitment was created and contains the new state
    let commit_state_pda = commit_state_pda_from_pubkey(&payer_delegated.pubkey());
    let commit_state_account = banks.get_account(commit_state_pda).await.unwrap().unwrap();
    assert!(commit_state_account.data.is_empty());

    // Assert the record about the commitment exists
    let commit_record_pda = commit_record_pda_from_pubkey(&payer_delegated.pubkey());
    let commit_record_account = banks.get_account(commit_record_pda).await.unwrap().unwrap();
    let commit_record = CommitRecord::try_from_bytes(&commit_record_account.data).unwrap();
    assert_eq!(commit_record.account, payer_delegated.pubkey());
    assert_eq!(commit_record.identity, validator.pubkey());
    assert_eq!(commit_record.slot, 100);

    let delegation_metadata_pda = delegation_metadata_pda_from_pubkey(&payer_delegated.pubkey());
    let delegation_metadata_account = banks
        .get_account(delegation_metadata_pda)
        .await
        .unwrap()
        .unwrap();
    let delegation_metadata =
        DelegationMetadata::try_from_slice(&delegation_metadata_account.data).unwrap();
    assert_eq!(delegation_metadata.is_undelegatable, true);
}

async fn setup_program_test_env() -> (BanksClient, Keypair, Keypair, Hash) {
    let mut program_test = ProgramTest::new("dlp", dlp::ID, processor!(dlp::process_instruction));
    program_test.prefer_bpf(true);

    // Setup the validator authority
    let validator_keypair = Keypair::from_bytes(&TEST_AUTHORITY).unwrap();
    program_test.add_account(
        validator_keypair.pubkey(),
        Account {
            lamports: 10 * LAMPORTS_PER_SOL,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup a delegated account
    let payer_alt = Keypair::from_bytes(&ON_CURVE_KEYPAIR).unwrap();
    program_test.add_account(
        payer_alt.pubkey(),
        Account {
            lamports: 10 * LAMPORTS_PER_SOL,
            data: vec![],
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the delegated record PDA
    let delegation_record_data =
        get_delegation_record_on_curve_data(validator_keypair.pubkey(), Some(LAMPORTS_PER_SOL));
    program_test.add_account(
        delegation_record_pda_from_pubkey(&payer_alt.pubkey()),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: delegation_record_data,
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the delegated account metadata PDA
    let delegation_metadata_data =
        get_delegation_metadata_data_on_curve(validator_keypair.pubkey(), None);
    program_test.add_account(
        delegation_metadata_pda_from_pubkey(&payer_alt.pubkey()),
        Account {
            lamports: Rent::default().minimum_balance(delegation_metadata_data.len()),
            data: delegation_metadata_data,
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the validator fees vault
    program_test.add_account(
        validator_fees_vault_pda_from_pubkey(&validator_keypair.pubkey()),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks, _, blockhash) = program_test.start().await;
    (banks, payer_alt, validator_keypair, blockhash)
}
