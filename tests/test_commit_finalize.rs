use dlp::args::{CommitFinalizeArgs, CommitStateArgs};
use dlp::pda::{
    delegation_metadata_pda_from_delegated_account, delegation_record_pda_from_delegated_account,
    validator_fees_vault_pda_from_validator,
};
use dlp::state::DelegationMetadata;
use solana_program::rent::Rent;
use solana_program::{hash::Hash, native_token::LAMPORTS_PER_SOL, system_program};
use solana_program_test::{BanksClient, ProgramTest};
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

use crate::fixtures::{
    get_delegation_metadata_data, get_delegation_record_data, DELEGATED_PDA_ID,
    DELEGATED_PDA_OWNER_ID, TEST_AUTHORITY,
};

mod fixtures;

#[tokio::test]
async fn test_commit_finalize() {
    // Setup
    let (banks, _, authority, blockhash) = setup_program_test_env().await;
    let new_state: Vec<u8> = (0..255).collect();

    let new_account_balance = 1_000_000;
    let commit_args = CommitFinalizeArgs {
        data: new_state.clone(),
        nonce: 1,
        allow_undelegation: 1,
        data_is_diff: 0,
        lamports: new_account_balance,
    };

    // Commit the state for the delegated account
    let ix = dlp::instruction_builder::commit_finalize(
        authority.pubkey(),
        DELEGATED_PDA_ID,
        DELEGATED_PDA_OWNER_ID,
        commit_args,
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&authority.pubkey()),
        &[&authority],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    println!("{:?}", res);
    assert!(res.is_ok());

    let delegated_account = banks.get_account(DELEGATED_PDA_ID).await.unwrap().unwrap();
    assert_eq!(delegated_account.data, new_state);

    let delegation_metadata_pda = delegation_metadata_pda_from_delegated_account(&DELEGATED_PDA_ID);
    let delegation_metadata_account = banks
        .get_account(delegation_metadata_pda)
        .await
        .unwrap()
        .unwrap();
    let delegation_metadata =
        DelegationMetadata::try_from_bytes_with_discriminator(&delegation_metadata_account.data)
            .unwrap();
    assert_eq!(delegation_metadata.is_undelegatable, true);
}

#[tokio::test]
async fn test_commit_out_of_order() {
    const OUTDATED_SLOT_ERR_MSG: &str =
        "transport transaction error: Error processing Instruction 0: custom program error: 0xc";

    // Setup
    let (banks, _, authority, blockhash) = setup_program_test_env().await;
    let new_state = vec![0, 1, 2, 9, 9, 9, 6, 7, 8, 9];

    let new_account_balance = 1_000_000;
    let commit_args = CommitStateArgs {
        data: new_state.clone(),
        nonce: 101,
        allow_undelegation: true,
        lamports: new_account_balance,
    };

    // Commit the state for the delegated account
    let ix = dlp::instruction_builder::commit_state(
        authority.pubkey(),
        DELEGATED_PDA_ID,
        DELEGATED_PDA_OWNER_ID,
        commit_args,
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&authority.pubkey()),
        &[&authority],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    assert_eq!(
        res.unwrap_err().to_string(),
        OUTDATED_SLOT_ERR_MSG.to_string()
    );
}

async fn setup_program_test_env() -> (BanksClient, Keypair, Keypair, Hash) {
    let mut program_test = ProgramTest::new("dlp", dlp::ID, None);
    program_test.prefer_bpf(true);

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

    // Setup a delegated PDA
    program_test.add_account(
        DELEGATED_PDA_ID,
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the delegated account metadata PDA
    let delegation_metadata_data = get_delegation_metadata_data(validator_keypair.pubkey(), None);
    program_test.add_account(
        delegation_metadata_pda_from_delegated_account(&DELEGATED_PDA_ID),
        Account {
            lamports: Rent::default().minimum_balance(delegation_metadata_data.len()),
            data: delegation_metadata_data,
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the delegated record PDA
    let delegation_record_data = get_delegation_record_data(validator_keypair.pubkey(), None);
    program_test.add_account(
        delegation_record_pda_from_delegated_account(&DELEGATED_PDA_ID),
        Account {
            lamports: Rent::default().minimum_balance(delegation_record_data.len()),
            data: delegation_record_data,
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the validator fees vault
    program_test.add_account(
        validator_fees_vault_pda_from_validator(&validator_keypair.pubkey()),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks, payer, blockhash) = program_test.start().await;
    (banks, payer, validator_keypair, blockhash)
}
