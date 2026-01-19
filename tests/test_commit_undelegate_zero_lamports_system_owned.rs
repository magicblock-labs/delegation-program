use dlp::args::CommitStateArgs;
use dlp::pda::{
    commit_record_pda_from_delegated_account, commit_state_pda_from_delegated_account,
    delegation_metadata_pda_from_delegated_account, delegation_record_pda_from_delegated_account,
    fees_vault_pda, validator_fees_vault_pda_from_validator,
};
use solana_program::rent::Rent;
use solana_program::{hash::Hash, native_token::LAMPORTS_PER_SOL, system_program};
use solana_program_test::{read_file, BanksClient, ProgramTest};
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

use crate::fixtures::{
    create_delegation_record_data, get_delegation_metadata_data, DELEGATED_PDA_ID,
    DELEGATED_PDA_OWNER_ID, TEST_AUTHORITY,
};

mod fixtures;

#[tokio::test]
async fn test_commit_and_undelegate_zero_lamports_system_owned_account() {
    // Setup
    let (banks, _, validator, blockhash) = setup_program_test_env().await;

    let delegated_account_before = banks.get_account(DELEGATED_PDA_ID).await.unwrap().unwrap();

    // Assert that the account is delegated, with some lamports and owned by dlp
    assert!(delegated_account_before.lamports > 0);
    assert_eq!(delegated_account_before.owner, dlp::id());

    let commit_args = CommitStateArgs {
        data: vec![],
        nonce: 1,
        allow_undelegation: true,
        lamports: 0,
    };

    let ix_commit = dlp::instruction_builder::commit_state(
        validator.pubkey(),
        DELEGATED_PDA_ID,
        DELEGATED_PDA_OWNER_ID,
        commit_args,
    );
    let tx_commit = Transaction::new_signed_with_payer(
        &[ix_commit],
        Some(&validator.pubkey()),
        &[&validator],
        blockhash,
    );
    let res_commit = banks.process_transaction(tx_commit).await;
    assert!(res_commit.is_ok());

    let ix_finalize = dlp::instruction_builder::finalize(validator.pubkey(), DELEGATED_PDA_ID);
    let ix_undelegate = dlp::instruction_builder::undelegate(
        validator.pubkey(),
        DELEGATED_PDA_ID,
        DELEGATED_PDA_OWNER_ID,
        validator.pubkey(),
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix_finalize, ix_undelegate],
        Some(&validator.pubkey()),
        &[&validator],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok());

    let commit_state_pda = commit_state_pda_from_delegated_account(&DELEGATED_PDA_ID);
    let commit_state_account = banks.get_account(commit_state_pda).await.unwrap();
    assert!(commit_state_account.is_none());

    let commit_record_pda = commit_record_pda_from_delegated_account(&DELEGATED_PDA_ID);
    let commit_record_account = banks.get_account(commit_record_pda).await.unwrap();
    assert!(commit_record_account.is_none());

    let delegation_record_pda = delegation_record_pda_from_delegated_account(&DELEGATED_PDA_ID);
    let delegation_record_account = banks.get_account(delegation_record_pda).await.unwrap();
    assert!(delegation_record_account.is_none());

    let delegation_metadata_pda = delegation_metadata_pda_from_delegated_account(&DELEGATED_PDA_ID);
    let delegation_metadata_account = banks.get_account(delegation_metadata_pda).await.unwrap();
    assert!(delegation_metadata_account.is_none());

    // Assert the account was closed
    let delegated_account_after = banks.get_account(DELEGATED_PDA_ID).await.unwrap();
    assert!(delegated_account_after.is_none());
}

async fn setup_program_test_env() -> (BanksClient, Keypair, Keypair, Hash) {
    let mut program_test = ProgramTest::new("dlp", dlp::ID, None);
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

    // Setup a delegated PDA that is system-owned (empty data) but initialized with
    // the minimum rent-exempt lamports. The "zero lamports" in the test name refers
    // to the commit args (lamports: 0), not the account balance itself.
    let rent_data_size: usize = 4;
    let data = vec![1u8; rent_data_size];
    program_test.add_account(
        DELEGATED_PDA_ID,
        Account {
            lamports: Rent::default().minimum_balance(rent_data_size),
            data,
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let delegation_record_data = create_delegation_record_data(
        validator.pubkey(),
        DELEGATED_PDA_OWNER_ID,
        Some(Rent::default().minimum_balance(rent_data_size)),
    );
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

    let delegation_metadata_data = get_delegation_metadata_data(validator.pubkey(), None);
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

    let data = read_file("tests/buffers/test_delegation.so");
    program_test.add_account(
        DELEGATED_PDA_OWNER_ID,
        Account {
            lamports: Rent::default().minimum_balance(data.len()).max(1),
            data,
            owner: solana_sdk::bpf_loader::id(),
            executable: true,
            rent_epoch: 0,
        },
    );

    program_test.add_account(
        fees_vault_pda(),
        Account {
            lamports: Rent::default().minimum_balance(0),
            data: vec![],
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    program_test.add_account(
        validator_fees_vault_pda_from_validator(&validator.pubkey()),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks, payer, blockhash) = program_test.start().await;
    (banks, payer, validator, blockhash)
}
