use dlp::solana_program;
use dlp_api::{
    args::{CommitFinalizeArgs, PreallocateBufferKind},
    pda::{
        delegation_metadata_pda_from_delegated_account,
        delegation_record_pda_from_delegated_account,
        validator_fees_vault_pda_from_validator,
    },
    state::DelegationMetadata,
};
use solana_program::{
    account_info::MAX_PERMITTED_DATA_INCREASE, hash::Hash,
    native_token::LAMPORTS_PER_SOL, rent::Rent,
};
use solana_program_test::{
    BanksClient, BanksClientError, BanksTransactionResultWithMetadata,
    ProgramTest,
};
use solana_sdk::{
    account::Account,
    instruction::InstructionError,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::{Transaction, TransactionError},
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

/// CommitFinalize writes the new state straight into the delegated account
/// (no intermediate `commit_state` PDA), so a target past
/// `MAX_PERMITTED_DATA_INCREASE` needs the delegated account itself grown
/// beforehand via `PreallocateBufferKind::DelegatedAccount`.
#[tokio::test]
async fn test_commit_finalize_from_buffer_large_with_preallocation() {
    let target_size = MAX_PERMITTED_DATA_INCREASE + 4_760; // 15_000
    let new_state = vec![7u8; target_size];

    let (banks, _, authority, blockhash) =
        setup_program_test_env(vec![], new_state.clone()).await;

    let state_buffer_pda =
        Pubkey::find_program_address(&[b"state_buffer"], &authority.pubkey()).0;

    let mut ixs = dlp_api::instruction_builder::preallocate_buffer_chunks(
        authority.pubkey(),
        DELEGATED_PDA_ID,
        PreallocateBufferKind::DelegatedAccount,
        0,
        target_size as u32,
    );
    let (ix, _pdas) = dlp_api::instruction_builder::commit_finalize_from_buffer(
        authority.pubkey(),
        DELEGATED_PDA_ID,
        state_buffer_pda,
        &mut CommitFinalizeArgs {
            commit_id: 1,
            allow_undelegation: true.into(),
            data_is_diff: false.into(),
            lamports: 1_000_000,
            bumps: Default::default(),
            reserved_padding: Default::default(),
        },
    );
    ixs.push(ix);

    let tx = Transaction::new_signed_with_payer(
        &ixs,
        Some(&authority.pubkey()),
        &[&authority],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok(), "{:?}", res);

    let delegated_account =
        banks.get_account(DELEGATED_PDA_ID).await.unwrap().unwrap();
    assert_eq!(delegated_account.data, new_state);
}

/// Regression test: `PreallocateBuffer(DelegatedAccount)` funds rent to grow
/// the delegated account ahead of CommitFinalize. CommitFinalize's own
/// settlement independently diffs `commit_lamports` (mirrored from the ER,
/// which already paid its own rent for the same growth) against
/// `delegation_record.lamports`. If preallocate doesn't keep that ledger
/// value in sync with what it actually transferred, settlement re-funds the
/// same growth a second time, and the delegated account ends up holding
/// roughly double the correct rent. Assert the final balance is exactly
/// `commit_lamports`, not double it.
#[tokio::test]
async fn test_commit_finalize_from_buffer_large_growth_does_not_double_pay_rent(
) {
    let target_size = MAX_PERMITTED_DATA_INCREASE + 4_760; // 15_000
    let new_state = vec![7u8; target_size];

    // Delegated account starts empty, with lamports exactly matching the
    // delegation record's ledger, so the settlement math below is exact.
    let initial_lamports = Rent::default().minimum_balance(0);
    let min_rent_target = Rent::default().minimum_balance(target_size);
    // Lamports the ER holds beyond bare rent-exemption for the new size --
    // settlement must add exactly this much, no more.
    let extra_on_er = 12_345u64;
    let commit_lamports = min_rent_target + extra_on_er;

    let (banks, _, authority, blockhash) =
        setup_program_test_env_with_lamports(
            vec![],
            new_state.clone(),
            initial_lamports,
        )
        .await;

    let state_buffer_pda =
        Pubkey::find_program_address(&[b"state_buffer"], &authority.pubkey()).0;

    let mut ixs = dlp_api::instruction_builder::preallocate_buffer_chunks(
        authority.pubkey(),
        DELEGATED_PDA_ID,
        PreallocateBufferKind::DelegatedAccount,
        0,
        target_size as u32,
    );
    let (ix, _pdas) = dlp_api::instruction_builder::commit_finalize_from_buffer(
        authority.pubkey(),
        DELEGATED_PDA_ID,
        state_buffer_pda,
        &mut CommitFinalizeArgs {
            commit_id: 1,
            allow_undelegation: true.into(),
            data_is_diff: false.into(),
            lamports: commit_lamports,
            bumps: Default::default(),
            reserved_padding: Default::default(),
        },
    );
    ixs.push(ix);

    let tx = Transaction::new_signed_with_payer(
        &ixs,
        Some(&authority.pubkey()),
        &[&authority],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok(), "{:?}", res);

    let delegated_account =
        banks.get_account(DELEGATED_PDA_ID).await.unwrap().unwrap();
    assert_eq!(delegated_account.data, new_state);
    assert_eq!(
        delegated_account.lamports, commit_lamports,
        "delegated account should hold exactly commit_lamports -- \
         preallocate's rent top-up and settlement's delta must not both \
         fund the same growth"
    );

    let delegation_record_account = banks
        .get_account(delegation_record_pda_from_delegated_account(
            &DELEGATED_PDA_ID,
        ))
        .await
        .unwrap()
        .unwrap();
    let delegation_record =
        dlp_api::state::DelegationRecord::try_from_bytes_with_discriminator(
            &delegation_record_account.data,
        )
        .unwrap();
    assert_eq!(delegation_record.lamports, commit_lamports);
}

#[tokio::test]
async fn test_commit_finalize_from_buffer_large_without_preallocation_fails() {
    let target_size = MAX_PERMITTED_DATA_INCREASE + 4_760; // 15_000
    let new_state = vec![7u8; target_size];

    let (banks, _, authority, blockhash) =
        setup_program_test_env(vec![], new_state).await;

    let state_buffer_pda =
        Pubkey::find_program_address(&[b"state_buffer"], &authority.pubkey()).0;

    let (ix, _pdas) = dlp_api::instruction_builder::commit_finalize_from_buffer(
        authority.pubkey(),
        DELEGATED_PDA_ID,
        state_buffer_pda,
        &mut CommitFinalizeArgs {
            commit_id: 1,
            allow_undelegation: true.into(),
            data_is_diff: false.into(),
            lamports: 1_000_000,
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
    let err = banks.process_transaction(tx).await.unwrap_err();
    // The un-preallocated delegated account starts at 0 bytes; CommitFinalize
    // writes directly into it, so its own `resize()` to the full 15_000-byte
    // target exceeds MAX_PERMITTED_DATA_INCREASE in a single instruction.
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

/// Distinct from the "no preallocation at all" case above: here the
/// delegated account *was* preallocated, just not far enough -- the gap left
/// to the actual target still exceeds `MAX_PERMITTED_DATA_INCREASE`.
/// CommitFinalize (unlike `commit_state_from_buffer`) has no "must be
/// preallocated to the exact size" check of its own -- it just resizes
/// directly -- so this is exercising the same native realloc cap, not a
/// dedicated dlp error.
#[tokio::test]
async fn test_commit_finalize_from_buffer_large_wrong_preallocated_size_fails()
{
    let target_size = 25_000usize;
    let new_state = vec![7u8; target_size];

    let (banks, _, authority, blockhash) =
        setup_program_test_env(vec![], new_state).await;

    let state_buffer_pda =
        Pubkey::find_program_address(&[b"state_buffer"], &authority.pubkey()).0;

    // Only send the first growth step (10_240 bytes) toward the 25_000
    // target, leaving a 14_760-byte gap -- still short of the target by more
    // than one instruction can bridge.
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

    let (ix, _pdas) = dlp_api::instruction_builder::commit_finalize_from_buffer(
        authority.pubkey(),
        DELEGATED_PDA_ID,
        state_buffer_pda,
        &mut CommitFinalizeArgs {
            commit_id: 1,
            allow_undelegation: true.into(),
            data_is_diff: false.into(),
            lamports: 1_000_000,
            bumps: Default::default(),
            reserved_padding: Default::default(),
        },
    );
    ixs.push(ix);

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

/// Like `setup_program_test_env`, but with the delegated account's lamports
/// (and matching delegation record ledger value) both set to
/// `delegated_lamports` -- needed to test settlement math precisely, where
/// the two must start in sync for the invariants being tested to mean
/// anything (unlike the fixed, unrelated values `setup_program_test_env`
/// uses, which are fine for tests that don't assert exact final lamports).
async fn setup_program_test_env_with_lamports(
    pda_data: Vec<u8>,
    pda_new_state: Vec<u8>,
    delegated_lamports: u64,
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
            lamports: delegated_lamports,
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
        Some(delegated_lamports),
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
