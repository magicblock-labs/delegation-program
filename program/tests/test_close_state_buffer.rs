use delegation_program::solana_program;
use delegation_program_interface as dpi;
use dpi::{
    instruction_builder::{close_state_buffer, write_state_buffer},
    pda::{pending_commitment_pda, state_buffer_pda},
    PendingCommitment, StateBuffer, WriteStateBufferArgs,
    PENDING_COMMITMENT_STATUS_ACTIVE, PENDING_COMMITMENT_STATUS_FINALIZED,
};
use solana_program::{
    native_token::LAMPORTS_PER_SOL, pubkey::Pubkey, rent::Rent,
};
use solana_program_test::{BanksClient, BanksClientError, ProgramTest};
use solana_sdk::{
    account::Account,
    instruction::{Instruction, InstructionError},
    signature::{Keypair, Signer},
    transaction::{Transaction, TransactionError},
};
use solana_sdk_ids::system_program;
use wheels::layout::{Decodable, Encodable};

struct CloseStateBufferTestContext {
    banks: BanksClient,
    transaction_payer: Keypair,
    buffer_payer: Keypair,
    authority: Keypair,
    delegated_account: Pubkey,
    commit_seq: u32,
}

#[tokio::test]
async fn test_close_state_buffer_returns_rent_and_allows_retry() {
    let context = setup_test(None).await;
    assert_ne!(context.buffer_payer.pubkey(), context.authority.pubkey());

    let payer_balance_before = context
        .banks
        .get_balance(context.buffer_payer.pubkey())
        .await
        .unwrap();
    write_buffer(&context, true).await.unwrap();

    let state_buffer = state_buffer_pda(
        &context.delegated_account,
        context.commit_seq,
        &context.authority.pubkey(),
    );
    let state_buffer_account = context
        .banks
        .get_account(state_buffer)
        .await
        .unwrap()
        .unwrap();
    let state = StateBuffer::decode(&state_buffer_account.data).unwrap();
    assert_eq!(*state.rent_payer(), context.buffer_payer.pubkey());
    assert_eq!(
        context
            .banks
            .get_balance(context.buffer_payer.pubkey())
            .await
            .unwrap(),
        payer_balance_before - state_buffer_account.lamports
    );

    submit_close(&context, close_instruction(&context))
        .await
        .unwrap();
    assert!(context
        .banks
        .get_account(state_buffer)
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        context
            .banks
            .get_balance(context.buffer_payer.pubkey())
            .await
            .unwrap(),
        payer_balance_before
    );

    submit_close(&context, close_instruction(&context))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_close_state_buffer_closes_partial_upload() {
    let context = setup_test(None).await;
    write_buffer(&context, false).await.unwrap();

    submit_close(&context, close_instruction(&context))
        .await
        .unwrap();

    assert!(context
        .banks
        .get_account(state_buffer_pda(
            &context.delegated_account,
            context.commit_seq,
            &context.authority.pubkey(),
        ))
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn test_close_state_buffer_validates_authority_and_rent_recipient() {
    {
        let context = setup_test(None).await;
        write_buffer(&context, true).await.unwrap();
        let mut instruction = close_instruction(&context);
        instruction.accounts[0].pubkey = context.buffer_payer.pubkey();

        assert!(submit_close_with_signers(
            &context,
            instruction,
            &[&context.transaction_payer, &context.buffer_payer],
        )
        .await
        .is_err());
    }

    {
        let context = setup_test(None).await;
        write_buffer(&context, true).await.unwrap();
        let mut instruction = close_instruction(&context);
        instruction.accounts[3].pubkey = Pubkey::new_unique();

        assert_custom_error(
            submit_close(&context, instruction).await.unwrap_err(),
            dpi::error::DlpError::InvalidStateBufferRentPayer,
        );
    }
}

#[tokio::test]
async fn test_close_state_buffer_rejects_live_commitment() {
    let context = setup_test(Some(PENDING_COMMITMENT_STATUS_ACTIVE)).await;
    write_buffer(&context, true).await.unwrap();

    assert_custom_error(
        submit_close(&context, close_instruction(&context))
            .await
            .unwrap_err(),
        dpi::error::DlpError::StateBufferInUse,
    );
}

#[tokio::test]
async fn test_close_state_buffer_allows_terminal_commitment() {
    let context = setup_test(Some(PENDING_COMMITMENT_STATUS_FINALIZED)).await;
    write_buffer(&context, true).await.unwrap();

    submit_close(&context, close_instruction(&context))
        .await
        .unwrap();
}

fn close_instruction(context: &CloseStateBufferTestContext) -> Instruction {
    close_state_buffer(
        context.authority.pubkey(),
        context.delegated_account,
        context.commit_seq,
        context.buffer_payer.pubkey(),
    )
    .unwrap()
}

async fn write_buffer(
    context: &CloseStateBufferTestContext,
    finalized: bool,
) -> Result<(), BanksClientError> {
    let chunk = if finalized { vec![1, 2] } else { vec![1] };
    let instruction = write_state_buffer(
        context.buffer_payer.pubkey(),
        context.authority.pubkey(),
        context.delegated_account,
        WriteStateBufferArgs {
            commit_seq: context.commit_seq,
            total_len: 2,
            offset: 0,
            chunk,
        },
    );
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&context.transaction_payer.pubkey()),
        &[
            &context.transaction_payer,
            &context.buffer_payer,
            &context.authority,
        ],
        context.banks.get_latest_blockhash().await.unwrap(),
    );

    context.banks.process_transaction(transaction).await
}

async fn submit_close(
    context: &CloseStateBufferTestContext,
    instruction: Instruction,
) -> Result<(), BanksClientError> {
    submit_close_with_signers(
        context,
        instruction,
        &[&context.transaction_payer, &context.authority],
    )
    .await
}

async fn submit_close_with_signers(
    context: &CloseStateBufferTestContext,
    instruction: Instruction,
    signers: &[&Keypair],
) -> Result<(), BanksClientError> {
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&context.transaction_payer.pubkey()),
        signers,
        context.banks.get_latest_blockhash().await.unwrap(),
    );

    context.banks.process_transaction(transaction).await
}

async fn setup_test(pending_status: Option<u8>) -> CloseStateBufferTestContext {
    let mut program_test = ProgramTest::default();
    program_test.prefer_bpf(true);
    program_test.add_program("delegation_program", dpi::ID, None);

    let buffer_payer = Keypair::new();
    let authority = Keypair::new();
    let delegated_account = Pubkey::new_unique();
    let commit_seq = 7;
    for pubkey in [buffer_payer.pubkey(), authority.pubkey()] {
        program_test.add_account(
            pubkey,
            Account {
                lamports: 10 * LAMPORTS_PER_SOL,
                data: vec![],
                owner: system_program::id(),
                executable: false,
                rent_epoch: 0,
            },
        );
    }
    program_test.add_account(
        delegated_account,
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: dpi::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    if let Some(status) = pending_status {
        let data = pending_commitment_data(
            delegated_account,
            commit_seq,
            authority.pubkey(),
            status,
        );
        program_test.add_account(
            pending_commitment_pda(&delegated_account, commit_seq),
            Account {
                lamports: Rent::default().minimum_balance(data.len()),
                data,
                owner: dpi::id(),
                executable: false,
                rent_epoch: 0,
            },
        );
    }

    let (banks, transaction_payer, _) = program_test.start().await;
    CloseStateBufferTestContext {
        banks,
        transaction_payer,
        buffer_payer,
        authority,
        delegated_account,
        commit_seq,
    }
}

fn pending_commitment_data(
    account: Pubkey,
    commit_seq: u32,
    authority: Pubkey,
    status: u8,
) -> Vec<u8> {
    PendingCommitment {
        discriminator: PendingCommitment::DISCRIMINATOR,
        status,
        delegation_cluster: Pubkey::new_unique(),
        operator_identity: authority,
        operator_bond: Pubkey::new_unique(),
        account_pubkey: account,
        commit_seq,
        delegation_record: Pubkey::new_unique(),
        da_pointer_hash: [0; 32],
        account_state_hash: [0; 32],
        data_hash: [0; 32],
        lamports: LAMPORTS_PER_SOL,
        owner: Pubkey::new_unique(),
        allow_undelegation: false,
        _pad_after_allow_undelegation: [0; 7],
        state_commitment_hash: [0; 32],
        verifier_registry: Pubkey::new_unique(),
        challenge_window_id: 0,
        posted_slot: 0,
        activation_slot: 0,
        challenge_window_end_slot: 0,
        approval_count: 0,
        approval_threshold: 1,
        resolver: Pubkey::new_unique(),
        min_challenger_stake: 0,
        challenger_reveal_timeout_slots: 0,
        match_penalty_bps: 0,
        active_challenge: None,
        resolved_state_source: None,
        er_slot: None,
        _pad_before_selected_verifiers: [0; 5],
        selected_verifiers: vec![],
    }
    .encode()
    .unwrap()
}

fn assert_custom_error(
    error: BanksClientError,
    expected: dpi::error::DlpError,
) {
    assert!(
        matches!(
            error,
            BanksClientError::TransactionError(
                TransactionError::InstructionError(
                    0,
                    InstructionError::Custom(code),
                ),
            ) if code == expected as u32
        ),
        "unexpected error: {error:?}"
    );
}
