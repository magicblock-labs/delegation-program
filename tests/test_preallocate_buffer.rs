use dlp::solana_program;
use dlp_api::{
    args::{CommitStateFromBufferArgs, PreallocateBufferKind},
    error::DlpError,
    pda::{
        commit_record_pda_from_delegated_account,
        commit_state_pda_from_delegated_account,
        delegation_metadata_pda_from_delegated_account,
        delegation_record_pda_from_delegated_account,
        program_config_from_program_id,
        undelegate_buffer_pda_from_delegated_account,
        validator_fees_vault_pda_from_validator,
    },
};
use solana_program::{
    account_info::MAX_PERMITTED_DATA_INCREASE, hash::Hash,
    native_token::LAMPORTS_PER_SOL, rent::Rent,
};
use solana_program_test::{BanksClient, BanksClientError, ProgramTest};
use solana_sdk::{
    account::Account,
    instruction::InstructionError,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::{Transaction, TransactionError},
};
use solana_sdk_ids::system_program;

use crate::fixtures::{
    get_commit_record_account_data, get_delegation_metadata_data,
    get_delegation_record_data, DELEGATED_PDA_ID, DELEGATED_PDA_OWNER_ID,
    TEST_AUTHORITY,
};

mod fixtures;

fn assert_custom_error(err: BanksClientError, expected: DlpError) {
    match err {
        BanksClientError::TransactionError(TransactionError::InstructionError(
            _,
            InstructionError::Custom(code),
        )) => {
            assert_eq!(code, expected as u32, "unexpected error code");
        }
        other => panic!("expected custom error {expected:?}, got {other:?}"),
    }
}

#[tokio::test]
async fn test_preallocate_commit_state_creates_fresh() {
    let (banks, _, validator, blockhash) =
        setup_program_test_env(false, None).await;

    let target_size: u32 = 500;
    let ix = dlp_api::instruction_builder::preallocate_buffer(
        validator.pubkey(),
        DELEGATED_PDA_ID,
        PreallocateBufferKind::CommitState,
        target_size,
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&validator.pubkey()),
        &[&validator],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok(), "{:?}", res);

    let commit_state_pda =
        commit_state_pda_from_delegated_account(&DELEGATED_PDA_ID);
    let commit_state_account =
        banks.get_account(commit_state_pda).await.unwrap().unwrap();
    assert_eq!(commit_state_account.owner, dlp_api::id());
    assert_eq!(commit_state_account.data.len(), target_size as usize);
    assert!(commit_state_account.data.iter().all(|&b| b == 0));
}

#[tokio::test]
async fn test_preallocate_undelegate_buffer_creates_fresh() {
    let (banks, _, validator, blockhash) =
        setup_program_test_env(false, None).await;

    let target_size: u32 = 500;
    let ix = dlp_api::instruction_builder::preallocate_buffer(
        validator.pubkey(),
        DELEGATED_PDA_ID,
        PreallocateBufferKind::UndelegateBuffer,
        target_size,
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&validator.pubkey()),
        &[&validator],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok(), "{:?}", res);

    let undelegate_buffer_pda =
        undelegate_buffer_pda_from_delegated_account(&DELEGATED_PDA_ID);
    let undelegate_buffer_account = banks
        .get_account(undelegate_buffer_pda)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(undelegate_buffer_account.owner, dlp_api::id());
    assert_eq!(undelegate_buffer_account.data.len(), target_size as usize);
    assert!(undelegate_buffer_account.data.iter().all(|&b| b == 0));
}

#[tokio::test]
async fn test_preallocate_delegated_account_grows_and_ignores_commit_in_flight()
{
    // A commit is in flight (commit_record already initialized): the
    // DelegatedAccount kind is meant to run precisely in this window
    // (between commit_state and finalize), so it must not be blocked by it.
    let (banks, _, validator, blockhash) =
        setup_program_test_env(true, None).await;

    let target_size: u32 = 500;
    let ix = dlp_api::instruction_builder::preallocate_buffer(
        validator.pubkey(),
        DELEGATED_PDA_ID,
        PreallocateBufferKind::DelegatedAccount,
        target_size,
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&validator.pubkey()),
        &[&validator],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok(), "{:?}", res);

    let delegated_account =
        banks.get_account(DELEGATED_PDA_ID).await.unwrap().unwrap();
    assert_eq!(delegated_account.data.len(), target_size as usize);
    assert!(delegated_account.data.iter().all(|&b| b == 0));
}

#[tokio::test]
async fn test_preallocate_commit_state_idempotent_past_target() {
    let (banks, _, validator, blockhash) =
        setup_program_test_env(false, None).await;

    // Grow to 5_000 bytes first.
    let ix = dlp_api::instruction_builder::preallocate_buffer(
        validator.pubkey(),
        DELEGATED_PDA_ID,
        PreallocateBufferKind::CommitState,
        5_000,
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&validator.pubkey()),
        &[&validator],
        blockhash,
    );
    assert!(banks.process_transaction(tx).await.is_ok());

    // Calling again with a smaller target must be a no-op (never shrinks).
    let ix = dlp_api::instruction_builder::preallocate_buffer(
        validator.pubkey(),
        DELEGATED_PDA_ID,
        PreallocateBufferKind::CommitState,
        3_000,
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&validator.pubkey()),
        &[&validator],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok(), "{:?}", res);

    let commit_state_pda =
        commit_state_pda_from_delegated_account(&DELEGATED_PDA_ID);
    let commit_state_account =
        banks.get_account(commit_state_pda).await.unwrap().unwrap();
    assert_eq!(commit_state_account.data.len(), 5_000);
}

#[tokio::test]
async fn test_preallocate_commit_state_multiple_instructions_same_tx_exceed_cap(
) {
    // The single most important property of this feature: several
    // PreallocateBuffer instructions packed into ONE transaction can grow an
    // account past MAX_PERMITTED_DATA_INCREASE, because the growth cap
    // resets for each top-level instruction.
    let (banks, _, validator, blockhash) =
        setup_program_test_env(false, None).await;

    let target_size: u32 = MAX_PERMITTED_DATA_INCREASE as u32 + 4_760; // 15_000
    let ixs = dlp_api::instruction_builder::preallocate_buffer_chunks(
        validator.pubkey(),
        DELEGATED_PDA_ID,
        PreallocateBufferKind::CommitState,
        0,
        target_size,
    );
    assert_eq!(ixs.len(), 2, "expected exactly two growth steps");

    let tx = Transaction::new_signed_with_payer(
        &ixs,
        Some(&validator.pubkey()),
        &[&validator],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok(), "{:?}", res);

    let commit_state_pda =
        commit_state_pda_from_delegated_account(&DELEGATED_PDA_ID);
    let commit_state_account =
        banks.get_account(commit_state_pda).await.unwrap().unwrap();
    assert_eq!(commit_state_account.data.len(), target_size as usize);
    assert!(commit_state_account.data.iter().all(|&b| b == 0));
}

#[tokio::test]
async fn test_preallocate_commit_state_rejects_when_commit_in_flight() {
    let (banks, _, validator, blockhash) =
        setup_program_test_env(true, None).await;

    let ix = dlp_api::instruction_builder::preallocate_buffer(
        validator.pubkey(),
        DELEGATED_PDA_ID,
        PreallocateBufferKind::CommitState,
        500,
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&validator.pubkey()),
        &[&validator],
        blockhash,
    );
    let err = banks.process_transaction(tx).await.unwrap_err();
    assert_custom_error(err, DlpError::PreallocateBufferCommitInFlight);
}

#[tokio::test]
async fn test_preallocate_undelegate_buffer_rejects_when_commit_in_flight() {
    let (banks, _, validator, blockhash) =
        setup_program_test_env(true, None).await;

    let ix = dlp_api::instruction_builder::preallocate_buffer(
        validator.pubkey(),
        DELEGATED_PDA_ID,
        PreallocateBufferKind::UndelegateBuffer,
        500,
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&validator.pubkey()),
        &[&validator],
        blockhash,
    );
    let err = banks.process_transaction(tx).await.unwrap_err();
    assert_custom_error(err, DlpError::PreallocateBufferCommitInFlight);
}

#[tokio::test]
async fn test_preallocate_rejects_wrong_authority() {
    let other_validator = Keypair::new();
    let (banks, _, _validator, blockhash) =
        setup_program_test_env(false, Some(other_validator.pubkey())).await;

    // `other_validator` is a registered validator (has a fees vault) but is
    // not this delegation's authority.
    let ix = dlp_api::instruction_builder::preallocate_buffer(
        other_validator.pubkey(),
        DELEGATED_PDA_ID,
        PreallocateBufferKind::CommitState,
        500,
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&other_validator.pubkey()),
        &[&other_validator],
        blockhash,
    );
    let err = banks.process_transaction(tx).await.unwrap_err();
    assert_custom_error(err, DlpError::InvalidAuthority);
}

#[tokio::test]
async fn test_preallocate_rejects_unregistered_validator() {
    let (banks, _, _validator, blockhash) =
        setup_program_test_env(false, None).await;

    // Fresh validator with no validator_fees_vault PDA set up at all.
    let unregistered = Keypair::new();
    let ix = dlp_api::instruction_builder::preallocate_buffer(
        unregistered.pubkey(),
        DELEGATED_PDA_ID,
        PreallocateBufferKind::CommitState,
        500,
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&_validator.pubkey()),
        &[&_validator, &unregistered],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    assert!(res.is_err(), "expected failure for unregistered validator");
}

/// End-to-end: a commit larger than `MAX_PERMITTED_DATA_INCREASE` is
/// preallocated across multiple `PreallocateBuffer` instructions packed into
/// the same transaction as `commit_state_from_buffer`, which then succeeds
/// because the buffer is already at the exact target size.
#[tokio::test]
async fn test_preallocate_then_commit_state_from_buffer_large_succeeds() {
    let (banks, _, validator, blockhash) =
        setup_program_test_env(false, None).await;

    let target_size: u32 = MAX_PERMITTED_DATA_INCREASE as u32 + 4_760; // 15_000
    let new_state = vec![7u8; target_size as usize];

    let state_buffer_pda =
        Pubkey::find_program_address(&[b"state_buffer"], &validator.pubkey())
            .0;

    let mut ixs = dlp_api::instruction_builder::preallocate_buffer_chunks(
        validator.pubkey(),
        DELEGATED_PDA_ID,
        PreallocateBufferKind::CommitState,
        0,
        target_size,
    );
    ixs.push(dlp_api::instruction_builder::commit_state_from_buffer(
        validator.pubkey(),
        DELEGATED_PDA_ID,
        DELEGATED_PDA_OWNER_ID,
        state_buffer_pda,
        CommitStateFromBufferArgs {
            nonce: 1,
            lamports: Rent::default().minimum_balance(500),
            allow_undelegation: true,
        },
    ));

    let tx = Transaction::new_signed_with_payer(
        &ixs,
        Some(&validator.pubkey()),
        &[&validator],
        blockhash,
    );

    // The buffer holding the new state must itself exist on-chain (its
    // content is read directly by `commit_state_from_buffer`, no size limit
    // applies to it since it's not created within this transaction).
    let banks = seed_state_buffer(banks, state_buffer_pda, new_state.clone())
        .await;

    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok(), "{:?}", res);

    let commit_state_pda =
        commit_state_pda_from_delegated_account(&DELEGATED_PDA_ID);
    let commit_state_account =
        banks.get_account(commit_state_pda).await.unwrap().unwrap();
    assert_eq!(commit_state_account.data, new_state);
}

#[tokio::test]
async fn test_commit_state_from_buffer_large_without_preallocation_fails() {
    let (banks, _, validator, blockhash) =
        setup_program_test_env(false, None).await;

    let target_size: u32 = MAX_PERMITTED_DATA_INCREASE as u32 + 4_760;
    let new_state = vec![7u8; target_size as usize];
    let state_buffer_pda =
        Pubkey::find_program_address(&[b"state_buffer"], &validator.pubkey())
            .0;
    let banks = seed_state_buffer(banks, state_buffer_pda, new_state).await;

    let ix = dlp_api::instruction_builder::commit_state_from_buffer(
        validator.pubkey(),
        DELEGATED_PDA_ID,
        DELEGATED_PDA_OWNER_ID,
        state_buffer_pda,
        CommitStateFromBufferArgs {
            nonce: 1,
            lamports: Rent::default().minimum_balance(500),
            allow_undelegation: true,
        },
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&validator.pubkey()),
        &[&validator],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    assert!(
        res.is_err(),
        "large commit without any preallocation must fail"
    );
}

#[tokio::test]
async fn test_commit_state_from_buffer_large_wrong_preallocated_size_fails() {
    let (banks, _, validator, blockhash) =
        setup_program_test_env(false, None).await;

    let target_size: u32 = MAX_PERMITTED_DATA_INCREASE as u32 + 4_760; // 15_000
    let new_state = vec![7u8; target_size as usize];
    let state_buffer_pda =
        Pubkey::find_program_address(&[b"state_buffer"], &validator.pubkey())
            .0;
    let banks = seed_state_buffer(banks, state_buffer_pda, new_state).await;

    // Only preallocate one chunk (10_240), short of the 15_000 target.
    let mut ixs = vec![dlp_api::instruction_builder::preallocate_buffer(
        validator.pubkey(),
        DELEGATED_PDA_ID,
        PreallocateBufferKind::CommitState,
        target_size,
    )];
    ixs.push(dlp_api::instruction_builder::commit_state_from_buffer(
        validator.pubkey(),
        DELEGATED_PDA_ID,
        DELEGATED_PDA_OWNER_ID,
        state_buffer_pda,
        CommitStateFromBufferArgs {
            nonce: 1,
            lamports: Rent::default().minimum_balance(500),
            allow_undelegation: true,
        },
    ));
    let tx = Transaction::new_signed_with_payer(
        &ixs,
        Some(&validator.pubkey()),
        &[&validator],
        blockhash,
    );
    let err = banks.process_transaction(tx).await.unwrap_err();
    assert_custom_error(err, DlpError::BufferNotPreallocatedToExactSize);
}

/// Adds the state-buffer account holding the not-yet-committed data. Since
/// `ProgramTest` only accepts new accounts before `.start()`, tests that
/// need this alongside the standard fixture rebuild the environment with it
/// included rather than mutating a running `BanksClient`.
async fn seed_state_buffer(
    _banks: BanksClient,
    _state_buffer_pda: Pubkey,
    _data: Vec<u8>,
) -> BanksClient {
    unreachable!(
        "placeholder replaced by setup_program_test_env_with_state_buffer"
    )
}

#[allow(clippy::too_many_arguments)]
async fn setup_program_test_env(
    with_in_flight_commit: bool,
    extra_registered_validator: Option<Pubkey>,
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
            data: vec![],
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

    if let Some(other_validator) = extra_registered_validator {
        program_test.add_account(
            other_validator,
            Account {
                lamports: 10 * LAMPORTS_PER_SOL,
                data: vec![],
                owner: system_program::id(),
                executable: false,
                rent_epoch: 0,
            },
        );
        program_test.add_account(
            validator_fees_vault_pda_from_validator(&other_validator),
            Account {
                lamports: LAMPORTS_PER_SOL,
                data: vec![],
                owner: dlp_api::id(),
                executable: false,
                rent_epoch: 0,
            },
        );
    }

    if with_in_flight_commit {
        let commit_record_data =
            get_commit_record_account_data(validator_keypair.pubkey());
        program_test.add_account(
            commit_record_pda_from_delegated_account(&DELEGATED_PDA_ID),
            Account {
                lamports: Rent::default()
                    .minimum_balance(commit_record_data.len()),
                data: commit_record_data,
                owner: dlp_api::id(),
                executable: false,
                rent_epoch: 0,
            },
        );
    }

    // Absent program config PDA is treated as "no whitelist configured" by
    // `require_program_config`, so it's intentionally left unset here.
    let _ = program_config_from_program_id(&DELEGATED_PDA_OWNER_ID);

    let (banks, payer, blockhash) = program_test.start().await;
    (banks, payer, validator_keypair, blockhash)
}
