use assertables::assert_ge;
use dlp::solana_program;
use dlp_api::{
    args::CommitFinalizeArgs,
    diff::compute_diff,
    error::DlpError,
    pda::{
        delegation_metadata_pda_from_delegated_account,
        delegation_record_pda_from_delegated_account,
        validator_fees_vault_pda_from_validator,
    },
    state::{DelegationMetadata, DelegationRecord},
};
use solana_program::{hash::Hash, native_token::LAMPORTS_PER_SOL, rent::Rent};
use solana_program_test::{
    BanksClient, BanksClientError, BanksTransactionResultWithMetadata,
    ProgramTest,
};
use solana_sdk::{
    account::Account,
    instruction::InstructionError,
    signature::{Keypair, Signer},
    transaction::{Transaction, TransactionError},
};
use solana_sdk_ids::system_program;

use crate::fixtures::{
    get_delegation_metadata_data, get_delegation_record_data, DELEGATED_PDA_ID,
    DELEGATED_PDA_OWNER_ID, TEST_AUTHORITY,
};

mod fixtures;

#[tokio::test]
async fn test_commit_finalize_data_perf() {
    run_test_commit_finalize(vec![0; 10240], vec![1; 10240], false, 1550).await;
}

#[tokio::test]
async fn test_commit_finalize_diff_perf() {
    run_test_commit_finalize(vec![0; 10240], vec![1; 10240], true, 1800).await;
}

async fn run_test_commit_finalize(
    old_state: Vec<u8>,
    new_state: Vec<u8>,
    data_is_diff: bool,
    max_expected_cu: u64,
) {
    // Setup
    let (banks, _, authority, blockhash, _record_lamports) =
        setup_program_test_env(old_state.clone()).await;

    let new_account_balance =
        solana_program::rent::Rent::default().minimum_balance(new_state.len());

    let (ix, pdas) = dlp_api::instruction_builder::commit_finalize(
        authority.pubkey(),
        DELEGATED_PDA_ID,
        DELEGATED_PDA_OWNER_ID,
        authority.pubkey(),
        &mut CommitFinalizeArgs {
            commit_id: 1,
            allow_undelegation: true.into(),
            data_is_diff: data_is_diff.into(),
            lamports: new_account_balance,
            bumps: Default::default(),
            reserved_padding: Default::default(),
        },
        &if data_is_diff {
            compute_diff(&old_state, &new_state).to_vec()
        } else {
            new_state.clone()
        },
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&authority.pubkey()),
        &[&authority],
        blockhash,
    );

    // execute CommitFinalize and validate CU performmace
    {
        let BanksTransactionResultWithMetadata {
            result: _,
            metadata,
        } = banks.process_transaction_with_metadata(tx).await.unwrap();

        let metadata = metadata.unwrap();

        assertables::assert_le!(
            metadata.compute_units_consumed,
            max_expected_cu
        );

        assert_eq!(
            metadata.log_messages.len(),
            3,
            "CommitFinalize must not log anything in OK scenario"
        );
    }

    let delegated_account =
        banks.get_account(DELEGATED_PDA_ID).await.unwrap().unwrap();
    assert_eq!(delegated_account.data, new_state);

    let delegation_metadata_account = banks
        .get_account(pdas.delegation_metadata)
        .await
        .unwrap()
        .unwrap();

    let delegation_metadata =
        DelegationMetadata::try_from_bytes_with_discriminator(
            &delegation_metadata_account.data,
        )
        .unwrap();

    assert_eq!(
        delegation_metadata.undelegation_requester,
        dlp_api::state::UndelegationRequester::Validator
    );
}

#[tokio::test]
async fn test_commit_finalize_out_of_order() {
    // Setup
    let (banks, _, authority, blockhash, _record_lamports) =
        setup_program_test_env(vec![]).await;
    let new_state = vec![0, 1, 2, 9, 9, 9, 6, 7, 8, 9];

    let new_account_balance = 1_000_000;

    let (ix, _pdas) = dlp_api::instruction_builder::commit_finalize(
        authority.pubkey(),
        DELEGATED_PDA_ID,
        DELEGATED_PDA_OWNER_ID,
        authority.pubkey(),
        &mut CommitFinalizeArgs {
            commit_id: 2, // this is the min value which will cause NonceOutOfOrder
            allow_undelegation: true.into(),
            data_is_diff: false.into(),
            lamports: new_account_balance,
            bumps: Default::default(),
            reserved_padding: Default::default(),
        },
        &new_state,
    );

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&authority.pubkey()),
        &[&authority],
        blockhash,
    );

    let BanksTransactionResultWithMetadata { result, metadata } =
        banks.process_transaction_with_metadata(tx).await.unwrap();

    let metadata = metadata.unwrap();

    let log = metadata
        .log_messages
        .iter()
        .find(|log| log.contains("require"))
        .unwrap();

    assert_eq!(
        log,
        "Program log: require_eq!(args.commit_id, prev_id + 1) failed: 2 == 1"
    );

    assert!(
        metadata
            .log_messages
            .iter()
            .any(|log| log.contains("NonceOutOfOrder")),
        "{:#?}",
        metadata
    );

    assert_eq!(
        result.unwrap_err().to_string(),
        "Error processing Instruction 0: custom program error: 0xc"
    );
}

async fn setup_program_test_env(
    pda_data: Vec<u8>,
) -> (BanksClient, Keypair, Keypair, Hash, u64) {
    setup_program_test_env_with_record_lamports(pda_data, LAMPORTS_PER_SOL)
        .await
}

async fn setup_program_test_env_with_record_lamports(
    pda_data: Vec<u8>,
    record_lamports: u64,
) -> (BanksClient, Keypair, Keypair, Hash, u64) {
    let mut program_test = ProgramTest::new("dlp", dlp::ID, None);
    program_test.prefer_bpf(true);

    let validator_keypair =
        crate::fixtures::keypair_from_bytes(&TEST_AUTHORITY);

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
            lamports: record_lamports,
            data: pda_data,
            owner: dlp_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the delegated account metadata PDA
    let delegation_metadata_data =
        get_delegation_metadata_data(validator_keypair.pubkey(), None);
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

    // Setup the delegated record PDA
    let delegation_record_data = get_delegation_record_data(
        validator_keypair.pubkey(),
        Some(record_lamports),
    );
    program_test.add_account(
        delegation_record_pda_from_delegated_account(&DELEGATED_PDA_ID),
        Account {
            lamports: Rent::default()
                .minimum_balance(delegation_record_data.len()),
            data: delegation_record_data,
            owner: dlp_api::id(),
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
            owner: dlp_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks, payer, blockhash) = program_test.start().await;
    (banks, payer, validator_keypair, blockhash, record_lamports)
}

#[tokio::test]
async fn test_commit_finalize_lamports_increase() {
    let initial_lamports = LAMPORTS_PER_SOL;
    let commit_lamports = initial_lamports + 1_000;

    let (banks, _, authority, blockhash, _record_lamports) =
        setup_program_test_env_with_record_lamports(
            vec![0; 8],
            initial_lamports,
        )
        .await;

    let (ix, pdas) = dlp_api::instruction_builder::commit_finalize(
        authority.pubkey(),
        DELEGATED_PDA_ID,
        DELEGATED_PDA_OWNER_ID,
        authority.pubkey(),
        &mut CommitFinalizeArgs {
            commit_id: 1,
            allow_undelegation: false.into(),
            data_is_diff: false.into(),
            lamports: commit_lamports,
            bumps: Default::default(),
            reserved_padding: Default::default(),
        },
        &vec![1; 8],
    );

    let before_validator_lamports = banks
        .get_account(authority.pubkey())
        .await
        .unwrap()
        .unwrap()
        .lamports;

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&authority.pubkey()),
        &[&authority],
        blockhash,
    );
    banks.process_transaction(tx).await.unwrap();

    let delegated_account =
        banks.get_account(DELEGATED_PDA_ID).await.unwrap().unwrap();
    assert_eq!(delegated_account.lamports, commit_lamports);

    let after_validator_lamports = banks
        .get_account(authority.pubkey())
        .await
        .unwrap()
        .unwrap()
        .lamports;

    assert_ne!(before_validator_lamports - after_validator_lamports, 0);
    assert_ge!(
        before_validator_lamports - after_validator_lamports,
        commit_lamports - initial_lamports
    );

    let fees_vault = banks
        .get_account(pdas.validator_fees_vault)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(fees_vault.lamports, LAMPORTS_PER_SOL);

    let delegation_record_account = banks
        .get_account(pdas.delegation_record)
        .await
        .unwrap()
        .unwrap();
    let delegation_record =
        DelegationRecord::try_from_bytes_with_discriminator(
            &delegation_record_account.data,
        )
        .unwrap();
    assert_eq!(delegation_record.lamports, commit_lamports);
}

#[tokio::test]
async fn test_commit_finalize_lamports_decrease() {
    let initial_lamports = LAMPORTS_PER_SOL;
    let commit_lamports = initial_lamports - 1_000;

    let (banks, _, authority, blockhash, _record_lamports) =
        setup_program_test_env_with_record_lamports(
            vec![0; 8],
            initial_lamports,
        )
        .await;

    let (ix, pdas) = dlp_api::instruction_builder::commit_finalize(
        authority.pubkey(),
        DELEGATED_PDA_ID,
        DELEGATED_PDA_OWNER_ID,
        authority.pubkey(),
        &mut CommitFinalizeArgs {
            commit_id: 1,
            allow_undelegation: false.into(),
            data_is_diff: false.into(),
            lamports: commit_lamports,
            bumps: Default::default(),
            reserved_padding: Default::default(),
        },
        &vec![2; 8],
    );

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&authority.pubkey()),
        &[&authority],
        blockhash,
    );
    banks.process_transaction(tx).await.unwrap();

    let delegated_account =
        banks.get_account(DELEGATED_PDA_ID).await.unwrap().unwrap();
    assert_eq!(delegated_account.lamports, commit_lamports);

    let delegation_record_account = banks
        .get_account(pdas.delegation_record)
        .await
        .unwrap()
        .unwrap();
    let delegation_record =
        DelegationRecord::try_from_bytes_with_discriminator(
            &delegation_record_account.data,
        )
        .unwrap();
    assert_eq!(delegation_record.lamports, commit_lamports);

    let fees_vault = banks
        .get_account(pdas.validator_fees_vault)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        fees_vault.lamports,
        LAMPORTS_PER_SOL + (initial_lamports - commit_lamports)
    );
}

#[tokio::test]
async fn test_commit_finalize_rejects_underfunded_account() {
    let data_len = 8usize;
    let rent_min = Rent::default().minimum_balance(data_len);
    let initial_lamports = rent_min + 1_000;
    let commit_lamports = rent_min - 1;

    let (banks, _, authority, blockhash, _record_lamports) =
        setup_program_test_env_with_record_lamports(
            vec![0; data_len],
            initial_lamports,
        )
        .await;

    let (ix, pdas) = dlp_api::instruction_builder::commit_finalize(
        authority.pubkey(),
        DELEGATED_PDA_ID,
        DELEGATED_PDA_OWNER_ID,
        authority.pubkey(),
        &mut CommitFinalizeArgs {
            commit_id: 1,
            allow_undelegation: false.into(),
            data_is_diff: false.into(),
            lamports: commit_lamports,
            bumps: Default::default(),
            reserved_padding: Default::default(),
        },
        &vec![3; data_len],
    );

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&authority.pubkey()),
        &[&authority],
        blockhash,
    );
    let err = banks.process_transaction(tx).await.unwrap_err();
    match err {
        BanksClientError::TransactionError(
            TransactionError::InstructionError(
                _,
                InstructionError::Custom(code),
            ),
        ) => {
            assert_eq!(code, DlpError::InsufficientRent as u32);
        }
        _ => panic!("unexpected error: {err:?}"),
    }

    let delegated_account =
        banks.get_account(DELEGATED_PDA_ID).await.unwrap().unwrap();
    assert_eq!(delegated_account.lamports, initial_lamports);

    let delegation_record_account = banks
        .get_account(pdas.delegation_record)
        .await
        .unwrap()
        .unwrap();
    let delegation_record =
        DelegationRecord::try_from_bytes_with_discriminator(
            &delegation_record_account.data,
        )
        .unwrap();
    assert_eq!(delegation_record.lamports, initial_lamports);
}
