use dlp::solana_program;
use dlp_api::{
    args::{CommitStateFromBufferArgs, PreallocateBufferKind},
    diff::compute_diff,
    error::DlpError,
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
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::{Transaction, TransactionError},
};
use solana_sdk_ids::system_program;

use crate::fixtures::{
    get_delegation_metadata_data, get_delegation_record_data, DELEGATED_PDA_ID,
    DELEGATED_PDA_OWNER_ID, TEST_AUTHORITY,
};

mod fixtures;

const NEW_STATE: [u8; 10] = [0, 1, 2, 9, 9, 9, 6, 7, 8, 9];

#[tokio::test]
async fn test_commit_new_state_from_buffer() {
    // Setup
    let (banks, _, authority, blockhash) =
        setup_program_test_env(vec![], NEW_STATE.to_vec()).await;
    let new_account_balance = 1_000_000;
    let state_buffer_pda =
        Pubkey::find_program_address(&[b"state_buffer"], &authority.pubkey()).0;

    let commit_args = CommitStateFromBufferArgs {
        nonce: 1,
        allow_undelegation: true,
        lamports: new_account_balance,
    };

    // Commit the state for the delegated account
    let ix = dlp_api::instruction_builder::commit_state_from_buffer(
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
    let commit_state_pda =
        commit_state_pda_from_delegated_account(&DELEGATED_PDA_ID);
    let commit_state_account =
        banks.get_account(commit_state_pda).await.unwrap().unwrap();
    assert_eq!(commit_state_account.data, NEW_STATE.to_vec());

    // Check that the commit has enough collateral to finalize the proposed state diff
    let delegated_account =
        banks.get_account(DELEGATED_PDA_ID).await.unwrap().unwrap();
    assert!(
        new_account_balance
            < commit_state_account.lamports + delegated_account.lamports
    );

    // Assert the record about the commitment exists
    let commit_record_pda =
        commit_record_pda_from_delegated_account(&DELEGATED_PDA_ID);
    let commit_record_account =
        banks.get_account(commit_record_pda).await.unwrap().unwrap();
    let commit_record = CommitRecord::try_from_bytes_with_discriminator(
        &commit_record_account.data,
    )
    .unwrap();
    assert_eq!(commit_record.account, DELEGATED_PDA_ID);
    assert_eq!(commit_record.identity, authority.pubkey());
    assert_eq!(commit_record.nonce, 1);

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
    assert_eq!(
        delegation_metadata.undelegation_requester,
        dlp_api::state::UndelegationRequester::Validator
    );
}

/// A commit_state target past `MAX_PERMITTED_DATA_INCREASE` needs the
/// commit_state PDA preallocated first via `PreallocateBufferKind::CommitState`.
#[tokio::test]
async fn test_commit_new_state_from_buffer_large_with_preallocation() {
    let target_size = MAX_PERMITTED_DATA_INCREASE + 4_760; // 15_000
    let new_state = vec![7u8; target_size];

    let (banks, _, authority, blockhash) =
        setup_program_test_env(vec![], new_state.clone()).await;
    let state_buffer_pda =
        Pubkey::find_program_address(&[b"state_buffer"], &authority.pubkey()).0;

    let mut ixs = dlp_api::instruction_builder::preallocate_buffer_chunks(
        authority.pubkey(),
        DELEGATED_PDA_ID,
        PreallocateBufferKind::CommitState,
        0,
        target_size as u32,
    );
    ixs.push(dlp_api::instruction_builder::commit_state_from_buffer(
        authority.pubkey(),
        DELEGATED_PDA_ID,
        DELEGATED_PDA_OWNER_ID,
        state_buffer_pda,
        CommitStateFromBufferArgs {
            nonce: 1,
            lamports: 1_000_000,
            allow_undelegation: true,
        },
    ));

    let tx = Transaction::new_signed_with_payer(
        &ixs,
        Some(&authority.pubkey()),
        &[&authority],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok(), "{:?}", res);

    let commit_state_pda =
        commit_state_pda_from_delegated_account(&DELEGATED_PDA_ID);
    let commit_state_account =
        banks.get_account(commit_state_pda).await.unwrap().unwrap();
    assert_eq!(commit_state_account.data, new_state);
}

#[tokio::test]
async fn test_commit_new_state_from_buffer_large_without_preallocation_fails() {
    let target_size = MAX_PERMITTED_DATA_INCREASE + 4_760; // 15_000
    let new_state = vec![7u8; target_size];

    let (banks, _, authority, blockhash) =
        setup_program_test_env(vec![], new_state).await;
    let state_buffer_pda =
        Pubkey::find_program_address(&[b"state_buffer"], &authority.pubkey()).0;

    let ix = dlp_api::instruction_builder::commit_state_from_buffer(
        authority.pubkey(),
        DELEGATED_PDA_ID,
        DELEGATED_PDA_OWNER_ID,
        state_buffer_pda,
        CommitStateFromBufferArgs {
            nonce: 1,
            lamports: 1_000_000,
            allow_undelegation: true,
        },
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&authority.pubkey()),
        &[&authority],
        blockhash,
    );
    let err = banks.process_transaction(tx).await.unwrap_err();
    // commit_state has no PDA to grow yet at all -- create_or_verify_preallocated_pda
    // requires it to already exist (initialized by PreallocateBuffer) once
    // the target exceeds MAX_PERMITTED_DATA_INCREASE, so this fails the
    // ownership check before it can even get to the size comparison.
    assert!(
        matches!(
            err,
            BanksClientError::TransactionError(
                TransactionError::InstructionError(
                    _,
                    InstructionError::InvalidAccountOwner,
                )
            )
        ),
        "expected InvalidAccountOwner, got {err:?}"
    );
}

/// Distinct from "no preallocation at all": the commit_state PDA *was*
/// preallocated, but not to the exact target size -- `commit_state_from_buffer`
/// (unlike `finalize`/`commit_finalize_from_buffer`) enforces an exact match
/// via `create_or_verify_preallocated_pda`, since it never resizes on its own.
#[tokio::test]
async fn test_commit_new_state_from_buffer_large_wrong_preallocated_size_fails()
{
    let target_size = MAX_PERMITTED_DATA_INCREASE + 4_760; // 15_000
    let new_state = vec![7u8; target_size];

    let (banks, _, authority, blockhash) =
        setup_program_test_env(vec![], new_state).await;
    let state_buffer_pda =
        Pubkey::find_program_address(&[b"state_buffer"], &authority.pubkey()).0;

    // Preallocate to a size other than the actual target.
    let mut ixs = vec![dlp_api::instruction_builder::preallocate_buffer(
        authority.pubkey(),
        DELEGATED_PDA_ID,
        PreallocateBufferKind::CommitState,
        target_size as u32 - 1,
    )];
    ixs.push(dlp_api::instruction_builder::commit_state_from_buffer(
        authority.pubkey(),
        DELEGATED_PDA_ID,
        DELEGATED_PDA_OWNER_ID,
        state_buffer_pda,
        CommitStateFromBufferArgs {
            nonce: 1,
            lamports: 1_000_000,
            allow_undelegation: true,
        },
    ));
    let tx = Transaction::new_signed_with_payer(
        &ixs,
        Some(&authority.pubkey()),
        &[&authority],
        blockhash,
    );
    let err = banks.process_transaction(tx).await.unwrap_err();
    assert_custom_error(err, DlpError::BufferNotPreallocatedToExactSize);
}

/// Builds a large, mostly-matching (base, changed) pair so their diff is
/// genuinely small despite the resulting state being large -- unlike diffing
/// against an empty account, where every byte is "new" and the diff ends up
/// roughly as big as the target itself.
fn large_base_and_small_diff_change(target_size: usize) -> (Vec<u8>, Vec<u8>) {
    let base = vec![1u8; target_size];
    let mut changed = base.clone();
    changed[0..50].fill(9);
    (base, changed)
}

/// The diff-based sibling of `commit_state_from_buffer`. The concern this
/// directly guards against: `create_or_verify_preallocated_pda`'s exact-size
/// check must compare against the *full* post-diff state length
/// (`DiffSet::changed_len()`), never the diff payload's own byte length --
/// those two are very different numbers here (a small diff producing a large
/// resulting state).
#[tokio::test]
async fn test_commit_diff_from_buffer_large_with_preallocation() {
    let target_size = MAX_PERMITTED_DATA_INCREASE + 4_760; // 15_000
    let (base, new_state) = large_base_and_small_diff_change(target_size);
    let diff = compute_diff(&base, &new_state);
    assert!(
        diff.len() < 1_000,
        "test setup expects a small diff payload despite the large target"
    );

    let (banks, _, authority, blockhash) =
        setup_program_test_env(base, diff.to_vec()).await;
    let diff_buffer_pda =
        Pubkey::find_program_address(&[b"state_buffer"], &authority.pubkey()).0;

    let mut ixs = dlp_api::instruction_builder::preallocate_buffer_chunks(
        authority.pubkey(),
        DELEGATED_PDA_ID,
        PreallocateBufferKind::CommitState,
        0,
        target_size as u32,
    );
    ixs.push(dlp_api::instruction_builder::commit_diff_from_buffer(
        authority.pubkey(),
        DELEGATED_PDA_ID,
        DELEGATED_PDA_OWNER_ID,
        diff_buffer_pda,
        CommitStateFromBufferArgs {
            nonce: 1,
            lamports: 1_000_000,
            allow_undelegation: true,
        },
    ));

    let tx = Transaction::new_signed_with_payer(
        &ixs,
        Some(&authority.pubkey()),
        &[&authority],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok(), "{:?}", res);

    let commit_state_pda =
        commit_state_pda_from_delegated_account(&DELEGATED_PDA_ID);
    let commit_state_account =
        banks.get_account(commit_state_pda).await.unwrap().unwrap();
    assert_eq!(commit_state_account.data, new_state);
}

/// Preallocating to the *diff's* length instead of the full post-diff state
/// length must be rejected -- if this ever passed, it would mean the exact-
/// size check regressed to comparing against the wrong number.
#[tokio::test]
async fn test_commit_diff_from_buffer_preallocated_to_diff_len_fails() {
    let target_size = MAX_PERMITTED_DATA_INCREASE + 4_760; // 15_000
    let (base, new_state) = large_base_and_small_diff_change(target_size);
    let diff = compute_diff(&base, &new_state);
    assert!(
        diff.len() < MAX_PERMITTED_DATA_INCREASE,
        "test setup expects a diff payload that fits in one prealloc step"
    );

    let (banks, _, authority, blockhash) =
        setup_program_test_env(base, diff.to_vec()).await;
    let diff_buffer_pda =
        Pubkey::find_program_address(&[b"state_buffer"], &authority.pubkey()).0;

    // Preallocate to the diff's own length, not the full target -- wrong on
    // purpose.
    let mut ixs = vec![dlp_api::instruction_builder::preallocate_buffer(
        authority.pubkey(),
        DELEGATED_PDA_ID,
        PreallocateBufferKind::CommitState,
        diff.len() as u32,
    )];
    ixs.push(dlp_api::instruction_builder::commit_diff_from_buffer(
        authority.pubkey(),
        DELEGATED_PDA_ID,
        DELEGATED_PDA_OWNER_ID,
        diff_buffer_pda,
        CommitStateFromBufferArgs {
            nonce: 1,
            lamports: 1_000_000,
            allow_undelegation: true,
        },
    ));
    let tx = Transaction::new_signed_with_payer(
        &ixs,
        Some(&authority.pubkey()),
        &[&authority],
        blockhash,
    );
    let err = banks.process_transaction(tx).await.unwrap_err();
    assert_custom_error(err, DlpError::BufferNotPreallocatedToExactSize);
}

fn assert_custom_error(err: BanksClientError, expected: DlpError) {
    match err {
        BanksClientError::TransactionError(
            TransactionError::InstructionError(
                _,
                InstructionError::Custom(code),
            ),
        ) => {
            assert_eq!(code, expected as u32, "unexpected error code");
        }
        other => panic!("expected custom error {expected:?}, got {other:?}"),
    }
}

async fn setup_program_test_env(
    delegated_pda_data: Vec<u8>,
    state_buffer_data: Vec<u8>,
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
            data: delegated_pda_data,
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

    // Setup a state buffer account
    program_test.add_account(
        Pubkey::find_program_address(
            &[b"state_buffer"],
            &validator_keypair.pubkey(),
        )
        .0,
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: state_buffer_data,
            owner: dlp_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks, payer, blockhash) = program_test.start().await;
    (banks, payer, validator_keypair, blockhash)
}
