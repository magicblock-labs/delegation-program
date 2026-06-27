use dlp::solana_program;
use dlp_api::{
    args::CommitFinalizeArgs,
    pda::{
        delegation_metadata_pda_from_delegated_account,
        delegation_record_pda_from_delegated_account,
        validator_fees_vault_pda_from_validator,
    },
    state::DelegationMetadata,
};
use solana_program::{hash::Hash, native_token::LAMPORTS_PER_SOL, rent::Rent};
use solana_program_test::{
    BanksClient, BanksTransactionResultWithMetadata, ProgramTest,
};
use solana_sdk::{
    account::Account,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use solana_sdk_ids::system_program;

use crate::fixtures::{
    get_delegation_metadata_data, get_delegation_record_data, DELEGATED_PDA_ID,
    TEST_AUTHORITY,
};

mod fixtures;

#[tokio::test]
async fn test_commit_finalize_from_buffer_perf() {
    // Setup
    let new_state = vec![1; 10240];

    let (banks, _, authority, blockhash) =
        setup_program_test_env(vec![0; 10240], new_state.clone()).await;

    let new_account_balance = 1_000_000;
    let state_buffer_pda =
        Pubkey::find_program_address(&[b"state_buffer"], &authority.pubkey()).0;

    let (ix, pdas) = dlp_api::instruction_builder::commit_finalize_from_buffer(
        authority.pubkey(),
        DELEGATED_PDA_ID,
        state_buffer_pda,
        &mut CommitFinalizeArgs {
            commit_id: 1,
            allow_undelegation: true.into(),
            data_is_diff: false.into(),
            lamports: new_account_balance,
            bumps: Default::default(),
            reserved_padding: Default::default(),
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

        assertables::assert_lt!(metadata.compute_units_consumed, 1450);

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
async fn test_commit_finalize_from_buffer_out_of_order() {
    // Setup
    let (banks, _, authority, blockhash) =
        setup_program_test_env(vec![], vec![0, 1, 2, 9, 9, 9, 6, 7, 8, 9])
            .await;

    let state_buffer_pda =
        Pubkey::find_program_address(&[b"state_buffer"], &authority.pubkey()).0;
    let new_account_balance = 1_000_000;

    let (ix, _pdas) = dlp_api::instruction_builder::commit_finalize_from_buffer(
        authority.pubkey(),
        DELEGATED_PDA_ID,
        state_buffer_pda,
        &mut CommitFinalizeArgs {
            commit_id: 2, // this is the min value which will cause NonceOutOfOrder
            allow_undelegation: true.into(),
            data_is_diff: false.into(),
            lamports: new_account_balance,
            bumps: Default::default(),
            reserved_padding: Default::default(),
        },
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
    pda_new_state: Vec<u8>,
) -> (BanksClient, Keypair, Keypair, Hash) {
    let mut program_test = ProgramTest::new("dlp", dlp_api::ID, None);
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
            lamports: LAMPORTS_PER_SOL,
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
    let delegation_record_data =
        get_delegation_record_data(validator_keypair.pubkey(), None);
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

    program_test.add_account(
        Pubkey::find_program_address(
            &[b"state_buffer"],
            &validator_keypair.pubkey(),
        )
        .0,
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: pda_new_state,
            owner: dlp_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks, payer, blockhash) = program_test.start().await;
    (banks, payer, validator_keypair, blockhash)
}
