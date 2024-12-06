use crate::fixtures::{
    create_delegation_metadata_data, create_delegation_record_data, TEST_AUTHORITY,
};
use dlp::args::DelegateEphemeralBalanceArgs;
use dlp::consts::{EPHEMERAL_BALANCE, FEES_VAULT};
use dlp::pda::{
    delegation_metadata_pda_from_pubkey, delegation_record_pda_from_pubkey,
    ephemeral_balance_from_payer, validator_fees_vault_pda_from_pubkey,
};
use solana_program::pubkey::Pubkey;
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
async fn test_top_up_ephemeral_balance() {
    // Setup
    let (mut banks, payer, _, blockhash) = setup_program_test_env().await;

    let ix = dlp::instruction_builder::top_up_ephemeral_balance(
        payer.pubkey(),
        payer.pubkey(),
        None,
        None,
    );
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[&payer], blockhash);
    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok());

    // Check account exists and it's owned by the system program
    let (ephemeral_balance, _) = Pubkey::find_program_address(
        &[EPHEMERAL_BALANCE, &payer.pubkey().to_bytes(), &[0]],
        &dlp::id(),
    );
    let balance_account = banks.get_account(ephemeral_balance).await.unwrap().unwrap();

    assert_eq!(balance_account.owner, system_program::id());
    assert!(balance_account.lamports > 0);
}

#[tokio::test]
async fn test_top_up_ephemeral_balance_and_delegate() {
    // Setup
    let (mut banks, payer, _, blockhash) = setup_program_test_env().await;

    // Top-up Ix
    let ix = dlp::instruction_builder::top_up_ephemeral_balance(
        payer.pubkey(),
        payer.pubkey(),
        None,
        None,
    );
    // Delegate ephemeral balance Ix
    let delegate_ix = dlp::instruction_builder::delegate_ephemeral_balance(
        payer.pubkey(),
        payer.pubkey(),
        DelegateEphemeralBalanceArgs::default(),
    );

    let tx = Transaction::new_signed_with_payer(
        &[ix, delegate_ix],
        Some(&payer.pubkey()),
        &[&payer],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok());
}

#[tokio::test]
async fn test_top_up_ephemeral_balance_for_pubkey() {
    // Setup
    let (mut banks, payer, _, blockhash) = setup_program_test_env().await;

    let pubkey = Keypair::new().pubkey();

    let ix = dlp::instruction_builder::top_up_ephemeral_balance(payer.pubkey(), pubkey, None, None);
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[&payer], blockhash);
    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok());

    // Check account exists and it's owned by the system program
    let ephemeral_balance = ephemeral_balance_from_payer(&pubkey, 0);
    let balance_account = banks.get_account(ephemeral_balance).await.unwrap().unwrap();

    assert_eq!(balance_account.owner, system_program::id());
    assert!(balance_account.lamports > 0);
}

#[tokio::test]
async fn test_top_up_ephemeral_balance_and_delegate_for_pubkey() {
    // Setup
    let (mut banks, payer, _, blockhash) = setup_program_test_env().await;

    let key = Keypair::new();
    let pubkey = key.pubkey();

    // Top-up Ix
    let ix = dlp::instruction_builder::top_up_ephemeral_balance(payer.pubkey(), pubkey, None, None);
    // Delegate ephemeral balance Ix
    let delegate_ix = dlp::instruction_builder::delegate_ephemeral_balance(
        payer.pubkey(),
        pubkey,
        DelegateEphemeralBalanceArgs::default(),
    );

    let tx = Transaction::new_signed_with_payer(
        &[ix, delegate_ix],
        Some(&payer.pubkey()),
        &[&payer, &key],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok());
}

#[tokio::test]
async fn test_undelegate_and_close() {
    // Setup
    let (mut banks, _, payer_alt, blockhash) = setup_program_test_env().await;

    let validator = Keypair::from_bytes(&TEST_AUTHORITY).unwrap();

    let (ephemeral_balance, _) = Pubkey::find_program_address(
        &[EPHEMERAL_BALANCE, &payer_alt.pubkey().to_bytes(), &[0]],
        &dlp::id(),
    );

    let prev_payer_lamports = banks
        .get_account(payer_alt.pubkey())
        .await
        .unwrap()
        .unwrap()
        .lamports;

    let ephemeral_balance_lamports = banks
        .get_account(ephemeral_balance)
        .await
        .unwrap()
        .unwrap()
        .lamports;

    // Undelegate ephemeral balance Ix
    let ix = dlp::instruction_builder::undelegate(
        validator.pubkey(),
        ephemeral_balance,
        dlp::id(),
        validator.pubkey(),
    );

    let ix_close = dlp::instruction_builder::close_ephemeral_balance(payer_alt.pubkey(), 0);

    let tx = Transaction::new_signed_with_payer(
        &[ix, ix_close],
        Some(&validator.pubkey()),
        &[&validator, &payer_alt],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok());

    // Assert that the ephemeral balance account is closed
    let (ephemeral_balance, _) = Pubkey::find_program_address(
        &[EPHEMERAL_BALANCE, &payer_alt.pubkey().to_bytes(), &[0]],
        &dlp::id(),
    );
    let balance_account = banks.get_account(ephemeral_balance).await.unwrap();

    assert!(balance_account.is_none());

    let payer_lamports = banks
        .get_account(payer_alt.pubkey())
        .await
        .unwrap()
        .unwrap()
        .lamports;
    assert_eq!(
        payer_lamports,
        prev_payer_lamports + ephemeral_balance_lamports
    );
}

async fn setup_program_test_env() -> (BanksClient, Keypair, Keypair, Hash) {
    let mut program_test = ProgramTest::new("dlp", dlp::ID, processor!(dlp::process_instruction));
    program_test.prefer_bpf(true);

    let payer_alt = Keypair::new();
    let validator = Keypair::from_bytes(&TEST_AUTHORITY).unwrap();

    program_test.add_account(
        payer_alt.pubkey(),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let ephemeral_balance = ephemeral_balance_from_payer(&payer_alt.pubkey(), 0);

    // Setup the delegated account PDA
    program_test.add_account(
        ephemeral_balance,
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
        create_delegation_record_data(validator.pubkey(), dlp::id(), Some(LAMPORTS_PER_SOL));
    program_test.add_account(
        delegation_record_pda_from_pubkey(&ephemeral_balance),
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
        vec![
            EPHEMERAL_BALANCE.to_vec(),
            payer_alt.pubkey().to_bytes().to_vec(),
            vec![0],
        ],
        true,
    );
    program_test.add_account(
        delegation_metadata_pda_from_pubkey(&ephemeral_balance),
        Account {
            lamports: Rent::default().minimum_balance(delegation_metadata_data.len()),
            data: delegation_metadata_data,
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

    // Setup the protocol fees vault
    program_test.add_account(
        Pubkey::find_program_address(&[FEES_VAULT], &dlp::id()).0,
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
        validator_fees_vault_pda_from_pubkey(&validator.pubkey()),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks, payer, blockhash) = program_test.start().await;
    (banks, payer, payer_alt, blockhash)
}
