use crate::fixtures::{
    create_delegation_metadata_data, create_delegation_record_data, get_commit_record_account_data,
    get_delegation_metadata_data, get_delegation_record_data, DELEGATED_PDA_ID,
    DELEGATED_PDA_OWNER_ID, TEST_AUTHORITY,
};
use borsh::{to_vec, BorshDeserialize, BorshSerialize};
use dlp::args::CallHandlerArgs;
use dlp::ephemeral_balance_seeds_from_payer;
use dlp::pda::{
    commit_record_pda_from_delegated_account, commit_state_pda_from_delegated_account,
    delegation_metadata_pda_from_delegated_account, delegation_record_pda_from_delegated_account,
    ephemeral_balance_pda_from_payer, fees_vault_pda, validator_fees_vault_pda_from_validator,
};
use solana_program::instruction::AccountMeta;
use solana_program::rent::Rent;
use solana_program::{hash::Hash, native_token::LAMPORTS_PER_SOL, system_program};
use solana_program_test::{processor, read_file, BanksClient, ProgramTest};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

mod fixtures;

// Mimic counter from test_delegation program
#[derive(BorshSerialize, BorshDeserialize)]
pub struct Counter {
    pub count: u64,
}

async fn setup_delegated_pda(program_test: &mut ProgramTest, authority_pubkey: &Pubkey) {
    let state = to_vec(&Counter { count: 100 }).unwrap();
    // Setup a delegated PDA
    program_test.add_account(
        DELEGATED_PDA_ID,
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: state,
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the delegation record PDA
    let delegation_record_data = get_delegation_record_data(*authority_pubkey, None);
    program_test.add_account(
        delegation_record_pda_from_delegated_account(&DELEGATED_PDA_ID),
        Account {
            lamports: Rent::default().minimum_balance(delegation_record_data.len()),
            data: delegation_record_data.clone(),
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the delegated account metadata PDA
    let delegation_metadata_data = get_delegation_metadata_data(*authority_pubkey, Some(true));
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
}

async fn setup_commit_state(program_test: &mut ProgramTest, authority_pubkey: &Pubkey) {
    // Setup the commit state PDA
    let commit_state = to_vec(&Counter { count: 101 }).unwrap();
    program_test.add_account(
        commit_state_pda_from_delegated_account(&DELEGATED_PDA_ID),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: commit_state,
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let commit_record_data = get_commit_record_account_data(*authority_pubkey);
    program_test.add_account(
        commit_record_pda_from_delegated_account(&DELEGATED_PDA_ID),
        Account {
            lamports: Rent::default().minimum_balance(commit_record_data.len()),
            data: commit_record_data,
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
}

async fn setup_invalid_escrow_account(program_test: &mut ProgramTest, authority_pubkey: &Pubkey) {
    let ephemeral_balance_pda = ephemeral_balance_pda_from_payer(&DELEGATED_PDA_ID, 0);

    // Setup the delegated account PDA
    program_test.add_account(
        ephemeral_balance_pda,
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the delegated record PDA
    let delegation_record_data =
        create_delegation_record_data(*authority_pubkey, dlp::id(), Some(LAMPORTS_PER_SOL));
    program_test.add_account(
        delegation_record_pda_from_delegated_account(&ephemeral_balance_pda),
        Account {
            lamports: Rent::default().minimum_balance(delegation_record_data.len()),
            data: delegation_record_data,
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the delegated account metadata PDA
    let delegation_metadata_data = create_delegation_metadata_data(
        *authority_pubkey,
        ephemeral_balance_seeds_from_payer!(DELEGATED_PDA_ID, 0),
        true,
    );
    program_test.add_account(
        delegation_metadata_pda_from_delegated_account(&ephemeral_balance_pda),
        Account {
            lamports: Rent::default().minimum_balance(delegation_metadata_data.len()),
            data: delegation_metadata_data,
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
}

async fn setup_delegated_ephemeral_balance(
    program_test: &mut ProgramTest,
    validator: &Keypair,
    payer: &Keypair,
) {
    let ephemeral_balance_pda = ephemeral_balance_pda_from_payer(&payer.pubkey(), 1);

    // Setup the delegated account PDA
    program_test.add_account(
        ephemeral_balance_pda,
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the delegated record PDA
    let delegation_record_data = create_delegation_record_data(
        validator.pubkey(),
        system_program::id(),
        Some(LAMPORTS_PER_SOL),
    );
    program_test.add_account(
        delegation_record_pda_from_delegated_account(&ephemeral_balance_pda),
        Account {
            lamports: Rent::default().minimum_balance(delegation_record_data.len()),
            data: delegation_record_data,
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the delegated account metadata PDA
    let delegation_metadata_data = create_delegation_metadata_data(
        validator.pubkey(),
        ephemeral_balance_seeds_from_payer!(payer.pubkey(), 0),
        true,
    );
    program_test.add_account(
        delegation_metadata_pda_from_delegated_account(&ephemeral_balance_pda),
        Account {
            lamports: Rent::default().minimum_balance(delegation_metadata_data.len()),
            data: delegation_metadata_data,
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
}

async fn setup_ephemeral_balance(program_test: &mut ProgramTest, payer: &Keypair) {
    let ephemeral_balance_pda = ephemeral_balance_pda_from_payer(&payer.pubkey(), 2);

    // Setup the delegated account PDA
    program_test.add_account(
        ephemeral_balance_pda,
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
}

async fn setup_program_test_env() -> (BanksClient, Keypair, Keypair, Hash) {
    let mut program_test = ProgramTest::new("dlp", dlp::ID, processor!(dlp::process_instruction));
    program_test.prefer_bpf(true);

    let payer = Keypair::new();
    let validator = Keypair::from_bytes(&TEST_AUTHORITY).unwrap();

    // Setup authority
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

    // Setup necessary accounts
    setup_delegated_pda(&mut program_test, &validator.pubkey()).await;
    setup_commit_state(&mut program_test, &validator.pubkey()).await;
    setup_invalid_escrow_account(&mut program_test, &validator.pubkey()).await;
    setup_delegated_ephemeral_balance(&mut program_test, &validator, &payer).await;
    setup_ephemeral_balance(&mut program_test, &payer).await;

    // Setup the protocol fees vault
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
    // Setup the validator fees vault
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

    // Setup program to test delegation
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

    let (banks, _, blockhash) = program_test.start().await;
    (banks, payer, validator, blockhash)
}

/// Test call_handler in finalize context
#[tokio::test]
async fn test_finalize_call_handler() {
    const PRIZE: u64 = LAMPORTS_PER_SOL / 1000;

    let (banks, payer, validator, blockhash) = setup_program_test_env().await;

    let transfer_destination = Keypair::new();
    let finalize_ix = dlp::instruction_builder::finalize(validator.pubkey(), DELEGATED_PDA_ID);
    let call_handler_ix = dlp::instruction_builder::call_handler(
        validator.pubkey(),
        DELEGATED_PDA_OWNER_ID, // destination program
        payer.pubkey(),         // escrow authority
        vec![
            AccountMeta::new(transfer_destination.pubkey(), false),
            AccountMeta::new(DELEGATED_PDA_ID, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        CallHandlerArgs {
            escrow_index: 2, // undelegated escrow index,
            data: to_vec(&PRIZE).unwrap(),
            context: dlp::args::Context::Commit,
        },
    );

    let tx = Transaction::new_signed_with_payer(
        &[finalize_ix, call_handler_ix],
        Some(&validator.pubkey()),
        &[&validator],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok());

    // Prize transferred
    let transfer_destination = banks
        .get_account(transfer_destination.pubkey())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(transfer_destination.lamports, PRIZE);
}

/// Test call_handler in finalize context
#[tokio::test]
async fn test_undelegate_call_handler() {
    const PRIZE: u64 = LAMPORTS_PER_SOL / 1000;

    let (banks, payer, validator, blockhash) = setup_program_test_env().await;

    let transfer_destination = Keypair::new();
    let finalize_ix = dlp::instruction_builder::finalize(validator.pubkey(), DELEGATED_PDA_ID);
    let undelegate_ix = dlp::instruction_builder::undelegate(
        validator.pubkey(),
        DELEGATED_PDA_ID,
        DELEGATED_PDA_OWNER_ID,
        validator.pubkey(),
    );
    let call_handler_ix = dlp::instruction_builder::call_handler(
        validator.pubkey(),
        DELEGATED_PDA_OWNER_ID, // destination program
        payer.pubkey(),         // escrow authority
        vec![
            AccountMeta::new(transfer_destination.pubkey(), false),
            AccountMeta::new(DELEGATED_PDA_ID, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        CallHandlerArgs {
            escrow_index: 2, // undelegated escrow index,
            data: to_vec(&PRIZE).unwrap(),
            context: dlp::args::Context::Undelegate,
        },
    );

    let tx = Transaction::new_signed_with_payer(
        &[finalize_ix, undelegate_ix, call_handler_ix],
        Some(&validator.pubkey()),
        &[&validator],
        blockhash,
    );

    let counter_before = banks.get_account(DELEGATED_PDA_ID).await.unwrap().unwrap();
    println!("counter before: {:?}", counter_before.data);
    let counter_before = Counter::try_from_slice(&counter_before.data).unwrap();
    println!("counter before: {}", counter_before.count);
    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok());

    // Prize transferred
    let transfer_destination = banks
        .get_account(transfer_destination.pubkey())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(transfer_destination.lamports, PRIZE);

    let counter_after = banks.get_account(DELEGATED_PDA_ID).await.unwrap().unwrap();
    let counter_after = Counter::try_from_slice(&counter_after.data).unwrap();
    // Committing state from count 100 to 101, and then increasing in handler on 1
    assert_eq!(counter_before.count + 2, counter_after.count);
}

/// Testing call_handler in finalize context with invalid escrow
#[tokio::test]
async fn test_finalize_invalid_escrow_call_handler() {
    // Setup
    let (banks, _, authority, blockhash) = setup_program_test_env().await;

    // Submit the finalize with handler tx
    let transfer_destination = Keypair::new();
    let finalize_ix = dlp::instruction_builder::finalize(authority.pubkey(), DELEGATED_PDA_ID);
    let call_handler_ix = dlp::instruction_builder::call_handler(
        authority.pubkey(),
        DELEGATED_PDA_OWNER_ID, // destination program
        DELEGATED_PDA_ID,
        vec![AccountMeta::new(transfer_destination.pubkey(), false)],
        CallHandlerArgs {
            escrow_index: 0,
            data: vec![],
            context: dlp::args::Context::Commit,
        },
    );
    let tx = Transaction::new_signed_with_payer(
        &[finalize_ix, call_handler_ix],
        Some(&authority.pubkey()),
        &[&authority],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    assert!(res
        .unwrap_err()
        .to_string()
        .contains("Invalid account owner"));
}

#[tokio::test]
async fn test_undelegate_invalid_escow_call_handler() {
    const PRIZE: u64 = LAMPORTS_PER_SOL / 1000;

    let (banks, _, authority, blockhash) = setup_program_test_env().await;

    // Submit the finalize with handler tx
    let destination = Keypair::new();
    let finalize_ix = dlp::instruction_builder::finalize(authority.pubkey(), DELEGATED_PDA_ID);
    let finalize_call_handler_ix = dlp::instruction_builder::call_handler(
        authority.pubkey(),
        DELEGATED_PDA_OWNER_ID, // handler program
        DELEGATED_PDA_ID,
        vec![AccountMeta::new(destination.pubkey(), false)],
        CallHandlerArgs {
            escrow_index: 0,
            data: vec![],
            context: dlp::args::Context::Commit,
        },
    );

    let undelegate_ix = dlp::instruction_builder::undelegate(
        authority.pubkey(),
        DELEGATED_PDA_ID,
        DELEGATED_PDA_OWNER_ID,
        authority.pubkey(),
    );
    let undelegate_call_handler_ix = dlp::instruction_builder::call_handler(
        authority.pubkey(),
        DELEGATED_PDA_OWNER_ID, // handler program
        DELEGATED_PDA_ID,
        vec![AccountMeta::new(destination.pubkey(), false)],
        CallHandlerArgs {
            escrow_index: 0,
            data: to_vec(&PRIZE).unwrap(),
            context: dlp::args::Context::Undelegate,
        },
    );
    let tx = Transaction::new_signed_with_payer(
        &[
            finalize_ix,
            finalize_call_handler_ix,
            undelegate_ix,
            undelegate_call_handler_ix,
        ],
        Some(&authority.pubkey()),
        &[&authority],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    assert!(res
        .unwrap_err()
        .to_string()
        .contains("Invalid account owner"));
}
