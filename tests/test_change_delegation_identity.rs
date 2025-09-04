use dlp::pda::delegation_record_pda_from_delegated_account;
use dlp::state::DelegationRecord;
use solana_program::bpf_loader_upgradeable;
use solana_program::rent::Rent;
use solana_program::{hash::Hash, pubkey::Pubkey, system_program};
use solana_program_test::{BanksClient, ProgramTest};
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::{
    account::Account, signature::Keypair, signature::Signer, transaction::Transaction,
};

use crate::fixtures::{get_delegation_record_data, DELEGATED_PDA_ID, TEST_AUTHORITY};

mod fixtures;

#[tokio::test]
async fn test_change_delegation_identity_success() {
    // Setup environment and initial accounts
    let (banks, payer, admin, blockhash) = setup_program_test_env().await;

    // Prepare inputs
    let delegated_account = DELEGATED_PDA_ID;
    let delegation_record_pda = delegation_record_pda_from_delegated_account(&delegated_account);

    // Choose a new identity
    let new_identity = Keypair::new().pubkey();

    // Build instruction data: discriminator (8 bytes) + new identity (32 bytes)
    let mut data = vec![4, 0, 0, 0, 0, 0, 0, 0];
    data.extend_from_slice(new_identity.as_ref());

    // Accounts:
    // 0: [signer]   admin (upgrade authority of the delegation program)
    // 1: []         program data account for the delegation program (BPF Upgradeable Loader ProgramData PDA)
    // 2: [writable] delegation record PDA for the delegated account
    // 3: []         delegated account (used to derive the PDA)
    let program_data =
        Pubkey::find_program_address(&[dlp::ID.as_ref()], &bpf_loader_upgradeable::id()).0;

    let accounts = vec![
        AccountMeta::new_readonly(admin.pubkey(), true),
        AccountMeta::new_readonly(program_data, false),
        AccountMeta::new(delegation_record_pda, false),
        AccountMeta::new_readonly(delegated_account, false),
    ];

    let ix = Instruction {
        program_id: dlp::ID,
        accounts,
        data,
    };

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &admin],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    assert!(
        res.is_ok(),
        "ChangeDelegationIdentity should succeed: {:?}",
        res
    );

    // Assert the DelegationRecord authority has changed
    let record_acc = banks
        .get_account(delegation_record_pda)
        .await
        .unwrap()
        .expect("delegation record account must exist");
    let record = DelegationRecord::try_from_bytes_with_discriminator(&record_acc.data).unwrap();
    assert_eq!(record.authority, new_identity);
}

async fn setup_program_test_env() -> (BanksClient, Keypair, Keypair, Hash) {
    let mut program_test = ProgramTest::new("dlp", dlp::ID, None);
    program_test.prefer_bpf(true);

    // Admin (upgrade authority) keypair is our TEST_AUTHORITY from fixtures
    let admin_keypair = Keypair::from_bytes(&TEST_AUTHORITY).unwrap();

    // Add payer with enough SOL
    let payer = Keypair::new();
    program_test.add_account(
        payer.pubkey(),
        Account {
            lamports: 10_000_000_000,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Add the delegated account (owned by the delegation program)
    program_test.add_account(
        DELEGATED_PDA_ID,
        Account {
            lamports: 1_000_000_000,
            data: vec![],
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Add the delegation record PDA for the delegated account
    let delegation_record_data = get_delegation_record_data(admin_keypair.pubkey(), None);
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

    // Add the ProgramData account for the delegation program with upgrade authority = admin_keypair
    let program_data_address =
        Pubkey::find_program_address(&[dlp::ID.as_ref()], &bpf_loader_upgradeable::id()).0;
    let program_data_state =
        solana_program::bpf_loader_upgradeable::UpgradeableLoaderState::ProgramData {
            slot: 0,
            upgrade_authority_address: Some(admin_keypair.pubkey()),
        };
    let program_data_bytes = bincode::serialize(&program_data_state).unwrap();
    program_test.add_account(
        program_data_address,
        Account {
            lamports: 1_000_000_000,
            data: program_data_bytes,
            owner: bpf_loader_upgradeable::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks, _pt_payer, blockhash) = program_test.start().await;
    (banks, payer, admin_keypair, blockhash)
}
