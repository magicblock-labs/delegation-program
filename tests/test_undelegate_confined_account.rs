use dlp::pda::{
    delegation_metadata_pda_from_delegated_account,
    delegation_record_pda_from_delegated_account,
};
use solana_program::{
    hash::Hash, native_token::LAMPORTS_PER_SOL, rent::Rent, system_program,
};
use solana_program_test::{read_file, BanksClient, ProgramTest};
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

use crate::fixtures::{
    create_delegation_record_data, get_delegation_metadata_data, DELEGATED_PDA,
    DELEGATED_PDA_ID, DELEGATED_PDA_OWNER_ID, TEST_AUTHORITY,
};

mod fixtures;

#[tokio::test]
async fn test_undelegate_confined_account() {
    // Setup
    let (banks, _payer, admin, blockhash) = setup_program_test_env().await;

    // Save the delegated account data before undelegating (should be preserved)
    let delegated_before =
        banks.get_account(DELEGATED_PDA_ID).await.unwrap().unwrap();
    let data_before = delegated_before.data.clone();

    // Submit the admin-only undelegate (confined) tx
    let ix = dlp::instruction_builder::undelegate_confined_account(
        admin.pubkey(),
        DELEGATED_PDA_ID,
        DELEGATED_PDA_OWNER_ID,
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&admin.pubkey()),
        &[&admin],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok());

    // Assert delegation PDAs were closed
    let delegation_record_pda =
        delegation_record_pda_from_delegated_account(&DELEGATED_PDA_ID);
    let delegation_record_account =
        banks.get_account(delegation_record_pda).await.unwrap();
    assert!(delegation_record_account.is_none());

    let delegation_metadata_pda =
        delegation_metadata_pda_from_delegated_account(&DELEGATED_PDA_ID);
    let delegation_metadata_account =
        banks.get_account(delegation_metadata_pda).await.unwrap();
    assert!(delegation_metadata_account.is_none());

    // Assert the delegated account is now owned by the original owner program
    let pda_account =
        banks.get_account(DELEGATED_PDA_ID).await.unwrap().unwrap();
    assert_eq!(pda_account.owner, DELEGATED_PDA_OWNER_ID);

    // Assert the delegated account data matches the pre-undelegation data
    assert_eq!(pda_account.data, data_before);
}

async fn setup_program_test_env() -> (BanksClient, Keypair, Keypair, Hash) {
    let mut program_test = ProgramTest::new("dlp", dlp::ID, None);
    program_test.prefer_bpf(true);

    // Admin is the DEFAULT_VALIDATOR_IDENTITY when unit_test_config feature is enabled
    let admin = Keypair::from_bytes(&TEST_AUTHORITY).unwrap();

    // Admin account
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

    // Delegated PDA (owned by dlp)
    program_test.add_account(
        DELEGATED_PDA_ID,
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: DELEGATED_PDA.into(),
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Delegation record PDA with authority = system program (confined) and owner = external program
    let delegation_record_data = create_delegation_record_data(
        system_program::id(),
        DELEGATED_PDA_OWNER_ID,
        None,
    );
    program_test.add_account(
        delegation_record_pda_from_delegated_account(&DELEGATED_PDA_ID),
        Account {
            lamports: Rent::default()
                .minimum_balance(delegation_record_data.len()),
            data: delegation_record_data,
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Delegation metadata PDA (undelegatable) with seeds used by external test program
    let delegation_metadata_data =
        get_delegation_metadata_data(admin.pubkey(), Some(true));
    program_test.add_account(
        delegation_metadata_pda_from_delegated_account(&DELEGATED_PDA_ID),
        Account {
            lamports: Rent::default()
                .minimum_balance(delegation_metadata_data.len()),
            data: delegation_metadata_data,
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // External owner program that handles EXTERNAL_UNDELEGATE
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

    let (banks, payer, blockhash) = program_test.start().await;
    (banks, payer, admin, blockhash)
}
