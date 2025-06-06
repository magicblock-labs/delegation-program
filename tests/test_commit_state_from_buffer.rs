use crate::fixtures::{
    get_delegation_metadata_data, get_delegation_record_data, DELEGATED_PDA_ID,
    DELEGATED_PDA_OWNER_ID, TEST_AUTHORITY,
};
use dlp::args::CommitStateFromBufferArgs;
use dlp::pda::{
    commit_record_pda_from_delegated_account, commit_state_pda_from_delegated_account,
    delegation_metadata_pda_from_delegated_account, delegation_record_pda_from_delegated_account,
    validator_fees_vault_pda_from_validator,
};
use dlp::state::{CommitRecord, DelegationMetadata};
use solana_program::rent::Rent;
use solana_program::{hash::Hash, native_token::LAMPORTS_PER_SOL, system_program};
use solana_program_test::{processor, BanksClient, ProgramTest};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

mod fixtures;

const NEW_STATE: [u8; 10] = [0, 1, 2, 9, 9, 9, 6, 7, 8, 9];

#[tokio::test]
async fn test_commit_new_state_from_buffer() {
    // Setup
    let (banks, _, authority, blockhash) = setup_program_test_env().await;
    let new_account_balance = 1_000_000;
    let state_buffer_pda = Pubkey::find_program_address(&[b"state_buffer"], &authority.pubkey()).0;

    let commit_args = CommitStateFromBufferArgs {
        slot: 100,
        allow_undelegation: true,
        lamports: new_account_balance,
    };

    // Commit the state for the delegated account
    let ix = dlp::instruction_builder::commit_state_from_buffer(
        authority.pubkey(),
        DELEGATED_PDA_ID,
        DELEGATED_PDA_OWNER_ID,
        state_buffer_pda,
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

    // Assert the state commitment was created and contains the new state
    let commit_state_pda = commit_state_pda_from_delegated_account(&DELEGATED_PDA_ID);
    let commit_state_account = banks.get_account(commit_state_pda).await.unwrap().unwrap();
    assert_eq!(commit_state_account.data, NEW_STATE.to_vec());

    // Check that the commit has enough collateral to finalize the proposed state diff
    let delegated_account = banks.get_account(DELEGATED_PDA_ID).await.unwrap().unwrap();
    assert!(new_account_balance < commit_state_account.lamports + delegated_account.lamports);

    // Assert the record about the commitment exists
    let commit_record_pda = commit_record_pda_from_delegated_account(&DELEGATED_PDA_ID);
    let commit_record_account = banks.get_account(commit_record_pda).await.unwrap().unwrap();
    let commit_record =
        CommitRecord::try_from_bytes_with_discriminator(&commit_record_account.data).unwrap();
    assert_eq!(commit_record.account, DELEGATED_PDA_ID);
    assert_eq!(commit_record.identity, authority.pubkey());
    assert_eq!(commit_record.slot, 100);

    let delegation_metadata_pda = delegation_metadata_pda_from_delegated_account(&DELEGATED_PDA_ID);
    let delegation_metadata_account = banks
        .get_account(delegation_metadata_pda)
        .await
        .unwrap()
        .unwrap();
    let delegation_metadata =
        DelegationMetadata::try_from_bytes_with_discriminator(&delegation_metadata_account.data)
            .unwrap();
    assert!(delegation_metadata.is_undelegatable);
}

async fn setup_program_test_env() -> (BanksClient, Keypair, Keypair, Hash) {
    let mut program_test = ProgramTest::new("dlp", dlp::ID, processor!(dlp::process_instruction));
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

    // Setup a state buffer account
    program_test.add_account(
        Pubkey::find_program_address(&[b"state_buffer"], &validator_keypair.pubkey()).0,
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: NEW_STATE.to_vec(),
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks, payer, blockhash) = program_test.start().await;
    (banks, payer, validator_keypair, blockhash)
}
