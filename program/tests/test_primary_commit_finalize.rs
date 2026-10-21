use delegation_program::solana_program;
use delegation_program_interface as dpi;
use dpi::{
    diff::compute_diff,
    instruction_builder::commit_finalize,
    pda::{delegation_cluster_pda, delegation_state_pda},
    CommitFinalizeArgs, CommitPayloadKind, DelegationCluster, DelegationState,
    UndelegationRequester,
};
use solana_program::{
    native_token::LAMPORTS_PER_SOL, pubkey::Pubkey, rent::Rent,
};
use solana_program_test::{BanksClient, BanksClientError, ProgramTest};
use solana_sdk::{
    account::Account,
    instruction::InstructionError,
    signature::{Keypair, Signer},
    transaction::{Transaction, TransactionError},
};
use solana_sdk_ids::system_program;
use wheels::layout::{Decodable, Encodable};

const INITIAL_DATA: &[u8] = b"initial account data";
const FULL_DATA: &[u8] = b"complete committed account data";
const INITIAL_LAMPORTS: u64 = LAMPORTS_PER_SOL;

struct CommitFinalizeTestContext {
    banks: BanksClient,
    payer: Keypair,
    current_validator: Keypair,
    delegated_account: Pubkey,
    delegation_state: Pubkey,
    delegation_cluster: Pubkey,
    original_owner: Pubkey,
}

#[tokio::test]
async fn test_commit_finalize_applies_full_data_and_diff() {
    let context =
        setup_commit_finalize_test(false, UndelegationRequester::None).await;

    submit_commit(
        &context,
        &context.current_validator,
        CommitFinalizeArgs {
            commit_id: 1,
            lamports: INITIAL_LAMPORTS + 500,
            owner: context.original_owner,
            allow_undelegation: false,
            payload_kind: CommitPayloadKind::FullData.value(),
            payload_alignment_padding: 0,
            payload: FULL_DATA.to_vec(),
        },
    )
    .await
    .unwrap();

    {
        let account = context
            .banks
            .get_account(context.delegated_account)
            .await
            .unwrap()
            .unwrap();
        let state_account = context
            .banks
            .get_account(context.delegation_state)
            .await
            .unwrap()
            .unwrap();
        let state = DelegationState::decode(&state_account.data).unwrap();

        assert_eq!(account.data, FULL_DATA);
        assert_eq!(account.lamports, INITIAL_LAMPORTS + 500);
        assert_eq!(state.last_commit_id(), 1);
        assert_eq!(state.lamports(), INITIAL_LAMPORTS + 500);
        assert_eq!(
            state.undelegation_requester(),
            UndelegationRequester::None.value()
        );
    }

    let changed_data = b"complete changed account data and tail";
    submit_commit(
        &context,
        &context.current_validator,
        CommitFinalizeArgs {
            commit_id: 2,
            lamports: INITIAL_LAMPORTS + 300,
            owner: context.original_owner,
            allow_undelegation: true,
            payload_kind: CommitPayloadKind::Diff.value(),
            payload_alignment_padding: 0,
            payload: compute_diff(FULL_DATA, changed_data).to_vec(),
        },
    )
    .await
    .unwrap();

    let account = context
        .banks
        .get_account(context.delegated_account)
        .await
        .unwrap()
        .unwrap();
    let state_account = context
        .banks
        .get_account(context.delegation_state)
        .await
        .unwrap()
        .unwrap();
    let state = DelegationState::decode(&state_account.data).unwrap();

    assert_eq!(account.data, changed_data);
    assert_eq!(account.lamports, INITIAL_LAMPORTS + 300);
    assert_eq!(state.last_commit_id(), 2);
    assert_eq!(state.lamports(), INITIAL_LAMPORTS + 300);
    assert_eq!(
        state.undelegation_requester(),
        UndelegationRequester::Validator.value()
    );
}

#[tokio::test]
async fn test_commit_finalize_rejects_out_of_order_commit() {
    let context =
        setup_commit_finalize_test(false, UndelegationRequester::None).await;
    let result = submit_commit(
        &context,
        &context.current_validator,
        full_data_args(&context, 2),
    )
    .await;

    assert_custom_error(
        result.unwrap_err(),
        dpi::error::DlpError::NonceOutOfOrder,
    );
}

#[tokio::test]
async fn test_commit_finalize_rejects_challenge_cluster() {
    let context =
        setup_commit_finalize_test(true, UndelegationRequester::None).await;

    assert!(submit_commit(
        &context,
        &context.current_validator,
        full_data_args(&context, 1),
    )
    .await
    .is_err());
}

#[tokio::test]
async fn test_commit_finalize_rejects_wrong_validator_and_owner() {
    let context =
        setup_commit_finalize_test(false, UndelegationRequester::None).await;

    assert_custom_error(
        submit_commit(&context, &context.payer, full_data_args(&context, 1))
            .await
            .unwrap_err(),
        dpi::error::DlpError::InvalidAuthority,
    );

    let mut args = full_data_args(&context, 1);
    args.owner = Pubkey::new_unique();
    assert!(submit_commit(&context, &context.current_validator, args)
        .await
        .is_err());
}

#[tokio::test]
async fn test_commit_finalize_requires_owner_undelegation_to_be_allowed() {
    let context =
        setup_commit_finalize_test(false, UndelegationRequester::OwnerProgram)
            .await;

    assert_custom_error(
        submit_commit(
            &context,
            &context.current_validator,
            full_data_args(&context, 1),
        )
        .await
        .unwrap_err(),
        dpi::error::DlpError::OwnerRequestedUndelegation,
    );
}

fn full_data_args(
    context: &CommitFinalizeTestContext,
    commit_id: u64,
) -> CommitFinalizeArgs {
    CommitFinalizeArgs {
        commit_id,
        lamports: INITIAL_LAMPORTS,
        owner: context.original_owner,
        allow_undelegation: false,
        payload_kind: CommitPayloadKind::FullData.value(),
        payload_alignment_padding: 0,
        payload: FULL_DATA.to_vec(),
    }
}

async fn submit_commit(
    context: &CommitFinalizeTestContext,
    validator: &Keypair,
    args: CommitFinalizeArgs,
) -> Result<(), BanksClientError> {
    let instruction = commit_finalize(
        validator.pubkey(),
        context.delegation_cluster,
        context.delegated_account,
        args,
    )
    .unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&context.payer.pubkey()),
        &[&context.payer, validator],
        context.banks.get_latest_blockhash().await.unwrap(),
    );

    context.banks.process_transaction(tx).await
}

async fn setup_commit_finalize_test(
    challenge_enabled: bool,
    undelegation_requester: UndelegationRequester,
) -> CommitFinalizeTestContext {
    let mut program_test = ProgramTest::default();
    program_test.prefer_bpf(true);
    program_test.add_program("delegation_program", dpi::ID, None);

    let current_validator = Keypair::new();
    let delegated_account = Pubkey::new_unique();
    let original_owner = Pubkey::new_unique();
    let cluster_id = Pubkey::new_unique();
    let delegation_cluster = delegation_cluster_pda(&cluster_id);
    let (_, cluster_bump) = Pubkey::find_program_address(
        &[dpi::pda::DELEGATION_CLUSTER_SEED, cluster_id.as_ref()],
        &dpi::id(),
    );
    let delegation_state = delegation_state_pda(&delegated_account);
    let (_, state_bump) = Pubkey::find_program_address(
        &[dpi::pda::DELEGATION_STATE_SEED, delegated_account.as_ref()],
        &dpi::id(),
    );

    program_test.add_account(
        current_validator.pubkey(),
        Account {
            lamports: 10 * LAMPORTS_PER_SOL,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
    program_test.add_account(
        delegated_account,
        Account {
            lamports: INITIAL_LAMPORTS,
            data: INITIAL_DATA.to_vec(),
            owner: dpi::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let cluster_data = DelegationCluster {
        discriminator: DelegationCluster::DISCRIMINATOR,
        bump: cluster_bump,
        cluster_id,
        config_authority: Pubkey::new_unique(),
        validator_rotation_authority: Pubkey::new_unique(),
        current_validator: current_validator.pubkey(),
        paused: false,
        protocol_fee_vault: Pubkey::new_unique(),
        challenge_config: challenge_enabled.then(|| dpi::ChallengeConfig {
            resolver: Pubkey::new_unique(),
            min_operator_bond: 1,
            min_verifier_bond: 1,
            min_challenger_stake: 1,
            challenge_window_slots: 1,
            operator_response_timeout_slots: 1,
            challenger_reveal_timeout_slots: 1,
            payout_timelock_slots: 1,
            verifiers_per_commitment: 1,
            approval_threshold: 1,
            max_window_extensions: 1,
            match_penalty_bps: 1,
        }),
    }
    .encode()
    .unwrap();
    program_test.add_account(
        delegation_cluster,
        Account {
            lamports: Rent::default().minimum_balance(cluster_data.len()),
            data: cluster_data,
            owner: dpi::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let state_data = DelegationState {
        discriminator: DelegationState::DISCRIMINATOR,
        bump: state_bump,
        delegated_account,
        delegation_cluster,
        original_owner,
        rent_payer: Pubkey::new_unique(),
        delegation_slot: 0,
        lamports: INITIAL_LAMPORTS,
        last_commit_id: 0,
        undelegation_requester: undelegation_requester.value(),
        delegated_account_bump: 0,
        seed_count: 0,
        encoded_seeds: vec![],
    }
    .encode()
    .unwrap();
    program_test.add_account(
        delegation_state,
        Account {
            lamports: Rent::default().minimum_balance(state_data.len()),
            data: state_data,
            owner: dpi::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks, payer, _) = program_test.start().await;
    CommitFinalizeTestContext {
        banks,
        payer,
        current_validator,
        delegated_account,
        delegation_state,
        delegation_cluster,
        original_owner,
    }
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
