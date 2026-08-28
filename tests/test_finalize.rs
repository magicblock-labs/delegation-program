use dlp::solana_program;
use dlp_api::{
    args::PreallocateBufferKind,
    pda::{
        commit_record_pda_from_delegated_account,
        commit_state_pda_from_delegated_account,
        delegation_metadata_pda_from_delegated_account,
        delegation_record_pda_from_delegated_account,
        validator_fees_vault_pda_from_validator,
    },
    state::{CommitRecord, DelegationMetadata},
};
use solana_program::{
    account_info::MAX_PERMITTED_DATA_INCREASE, hash::Hash,
    native_token::LAMPORTS_PER_SOL, rent::Rent,
};
use solana_program_test::{BanksClient, BanksClientError, ProgramTest};
use solana_sdk::{
    account::Account,
    instruction::InstructionError,
    signature::{Keypair, Signer},
    transaction::{Transaction, TransactionError},
};
use solana_sdk_ids::system_program;

use crate::fixtures::{
    get_commit_record_account_data, get_delegation_metadata_data,
    get_delegation_record_data, COMMIT_NEW_STATE_ACCOUNT_DATA,
    DELEGATED_PDA_ID, TEST_AUTHORITY,
};

mod fixtures;

#[tokio::test]
async fn test_finalize() {
    // Setup
    let (banks, _, authority, blockhash) =
        setup_program_test_env(COMMIT_NEW_STATE_ACCOUNT_DATA.into()).await;

    // Retrieve the accounts
    let delegation_record_pda =
        delegation_record_pda_from_delegated_account(&DELEGATED_PDA_ID);
    let commit_state_pda =
        commit_state_pda_from_delegated_account(&DELEGATED_PDA_ID);
    let commit_record_pda =
        commit_record_pda_from_delegated_account(&DELEGATED_PDA_ID);

    // Commit state record data
    let commit_record =
        banks.get_account(commit_record_pda).await.unwrap().unwrap();
    let commit_record =
        CommitRecord::try_from_bytes_with_discriminator(&commit_record.data)
            .unwrap();

    // Save the new state data before finalizing
    let new_state_before_finalize =
        banks.get_account(commit_state_pda).await.unwrap().unwrap();
    let new_state_data_before_finalize = new_state_before_finalize.data.clone();

    // Submit the finalize tx
    let ix = dlp_api::instruction_builder::finalize(
        authority.pubkey(),
        DELEGATED_PDA_ID,
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

    // Assert the state_diff was closed
    let commit_state_account =
        banks.get_account(commit_state_pda).await.unwrap();
    assert!(commit_state_account.is_none());

    // Assert the delegation_record was not closed
    let delegation_record =
        banks.get_account(delegation_record_pda).await.unwrap();
    assert!(delegation_record.is_some());

    // Assert the commit_record_pda was closed
    let commit_record_account =
        banks.get_account(commit_record_pda).await.unwrap();
    assert!(commit_record_account.is_none());

    // Assert that the account owner is still the delegation program
    let pda_account =
        banks.get_account(DELEGATED_PDA_ID).await.unwrap().unwrap();
    assert!(pda_account.owner.eq(&dlp_api::id()));

    // Assert the delegated account contains the data from the new state
    assert_eq!(new_state_data_before_finalize, pda_account.data);

    // Assert the delegation metadata contains the correct slot of the commitment
    let delegation_metadata_pda =
        delegation_metadata_pda_from_delegated_account(&DELEGATED_PDA_ID);
    let delegation_metadata_account = banks
        .get_account(delegation_metadata_pda)
        .await
        .unwrap()
        .unwrap();
    let delegation_metadata =
        DelegationMetadata::try_from_bytes_with_discriminator(
            &delegation_metadata_account.data,
        )
        .unwrap();
    assert_eq!(commit_record.nonce, delegation_metadata.last_commit_id);
}

/// Finalize copies `commit_state`'s full content straight into the delegated
/// account (`delegated_account.resize(commit_state_data.len())`), so a
/// target past `MAX_PERMITTED_DATA_INCREASE` needs the delegated account
/// itself grown beforehand via `PreallocateBufferKind::DelegatedAccount`.
#[tokio::test]
async fn test_finalize_large_with_preallocation() {
    let target_size = MAX_PERMITTED_DATA_INCREASE + 4_760; // 15_000
    let new_state = vec![7u8; target_size];

    let (banks, _, authority, blockhash) =
        setup_program_test_env(new_state.clone()).await;

    let mut ixs = dlp_api::instruction_builder::preallocate_buffer_chunks(
        authority.pubkey(),
        DELEGATED_PDA_ID,
        PreallocateBufferKind::DelegatedAccount,
        0,
        target_size as u32,
    );
    ixs.push(dlp_api::instruction_builder::finalize(
        authority.pubkey(),
        DELEGATED_PDA_ID,
    ));

    let tx = Transaction::new_signed_with_payer(
        &ixs,
        Some(&authority.pubkey()),
        &[&authority],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok(), "{:?}", res);

    // The delegated account grew to the full preallocated size and now holds
    // the committed state's content exactly.
    let delegated_account =
        banks.get_account(DELEGATED_PDA_ID).await.unwrap().unwrap();
    assert_eq!(delegated_account.data, new_state);

    // finalize closes both the commit_state and commit_record PDAs once
    // their content has been applied, same as for a small-account finalize.
    let commit_state_pda =
        commit_state_pda_from_delegated_account(&DELEGATED_PDA_ID);
    assert!(banks.get_account(commit_state_pda).await.unwrap().is_none());
    let commit_record_pda =
        commit_record_pda_from_delegated_account(&DELEGATED_PDA_ID);
    assert!(banks
        .get_account(commit_record_pda)
        .await
        .unwrap()
        .is_none());
}

/// The delegated account *was* preallocated, just not far enough -- the gap
/// left to the actual target still exceeds `MAX_PERMITTED_DATA_INCREASE`.
/// `finalize` has no "must be preallocated to the exact size" check of its
/// own -- it just resizes directly -- so this exercises the native realloc
/// cap, not a dedicated dlp error.
#[tokio::test]
async fn test_finalize_large_wrong_preallocated_size_fails() {
    let target_size = 25_000usize;
    let new_state = vec![7u8; target_size];

    let (banks, _, authority, blockhash) =
        setup_program_test_env(new_state).await;

    // Only send the first growth step (10_240 bytes) toward the 25_000
    // target, leaving a 14_760-byte gap.
    let all_steps = dlp_api::instruction_builder::preallocate_buffer_chunks(
        authority.pubkey(),
        DELEGATED_PDA_ID,
        PreallocateBufferKind::DelegatedAccount,
        0,
        target_size as u32,
    );
    assert!(
        all_steps.len() > 1,
        "test setup expects the full growth to require multiple steps"
    );
    let mut ixs = vec![all_steps[0].clone()];
    ixs.push(dlp_api::instruction_builder::finalize(
        authority.pubkey(),
        DELEGATED_PDA_ID,
    ));

    let tx = Transaction::new_signed_with_payer(
        &ixs,
        Some(&authority.pubkey()),
        &[&authority],
        blockhash,
    );
    let err = banks.process_transaction(tx).await.unwrap_err();
    assert!(
        matches!(
            err,
            BanksClientError::TransactionError(
                TransactionError::InstructionError(
                    _,
                    InstructionError::InvalidRealloc,
                )
            )
        ),
        "expected InvalidRealloc, got {err:?}"
    );
}

async fn setup_program_test_env(
    commit_state_data: Vec<u8>,
) -> (BanksClient, Keypair, Keypair, Hash) {
    let mut program_test = ProgramTest::new("dlp", dlp_api::ID, None);
    program_test.prefer_bpf(true);

    let authority = crate::fixtures::keypair_from_bytes(&TEST_AUTHORITY);

    program_test.add_account(
        authority.pubkey(),
        Account {
            lamports: LAMPORTS_PER_SOL,
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
            owner: dlp_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the delegation record PDA
    let delegation_record_data =
        get_delegation_record_data(authority.pubkey(), None);
    program_test.add_account(
        delegation_record_pda_from_delegated_account(&DELEGATED_PDA_ID),
        Account {
            lamports: Rent::default()
                .minimum_balance(delegation_record_data.len()),
            data: delegation_record_data.clone(),
            owner: dlp_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the delegated account metadata PDA
    let delegation_metadata_data =
        get_delegation_metadata_data(authority.pubkey(), None);
    program_test.add_account(
        delegation_metadata_pda_from_delegated_account(&DELEGATED_PDA_ID),
        Account {
            lamports: Rent::default()
                .minimum_balance(delegation_metadata_data.len()),
            data: delegation_metadata_data,
            owner: dlp_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the commit state PDA
    program_test.add_account(
        commit_state_pda_from_delegated_account(&DELEGATED_PDA_ID),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: commit_state_data,
            owner: dlp_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let commit_record_data = get_commit_record_account_data(authority.pubkey());
    program_test.add_account(
        commit_record_pda_from_delegated_account(&DELEGATED_PDA_ID),
        Account {
            lamports: Rent::default().minimum_balance(commit_record_data.len()),
            data: commit_record_data,
            owner: dlp_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the validator fees vault
    program_test.add_account(
        validator_fees_vault_pda_from_validator(&authority.pubkey()),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: dlp_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks, payer, blockhash) = program_test.start().await;
    (banks, payer, authority, blockhash)
}
