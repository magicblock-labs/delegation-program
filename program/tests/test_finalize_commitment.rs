use delegation_program_interface as dpi;
use dpi::{
    pda::{
        delegation_metadata_pda_from_delegated_account,
        delegation_record_pda_from_delegated_account,
    },
    state::{DelegationMetadata, DelegationRecord},
    v2::{
        instruction_builder::{
            approve_commitment, finalize_commitment, propose_commitment,
            register_operator, register_verifier, update_verifier_registry,
            write_state_buffer,
        },
        pda::pending_commitment_pda,
        PendingCommitment, ProposeCommitmentArgs, RegisterOperatorArgs,
        RegisterVerifierArgs, UpdateVerifierRegistryArgs,
        VerifierRegistryAction, WriteStateBufferArgs,
        PENDING_COMMITMENT_STATUS_FINALIZED,
    },
};
use solana_program::native_token::LAMPORTS_PER_SOL;
use solana_program_test::{
    BanksClientError, ProgramTest, ProgramTestBanksClientExt,
    ProgramTestContext,
};
use solana_sdk::{
    account::{Account, AccountSharedData},
    hash::Hash,
    instruction::Instruction,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use solana_sdk_ids::system_program;
use wheels::layout::Decodable;

mod fixtures;

use crate::fixtures::{
    create_delegation_metadata_data, create_delegation_record_data,
    v2::{challenge_cluster_args, initialize_delegation_cluster_with_identity},
};

#[tokio::test]
async fn test_finalize_commitment() {
    let mut env =
        setup_finalize_commitment_env(LAMPORTS_PER_SOL, LAMPORTS_PER_SOL).await;

    propose(&mut env).await.unwrap();
    approve(&mut env).await.unwrap();
    warp_past_challenge_window(&mut env).await;
    finalize(&mut env).await.unwrap();

    let pending_commitment = read_pending_commitment(&mut env).await;
    assert_eq!(
        pending_commitment.status,
        PENDING_COMMITMENT_STATUS_FINALIZED
    );

    let delegated_account = env
        .context
        .banks_client
        .get_account(env.delegated_account)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(delegated_account.data, env.final_state_data);
    assert_eq!(delegated_account.lamports, env.committed_lamports);

    let delegation_record = read_delegation_record(&mut env).await;
    assert_eq!(delegation_record.lamports, env.committed_lamports);

    let delegation_metadata = read_delegation_metadata(&mut env).await;
    assert_eq!(delegation_metadata.last_commit_id, env.commit_id);
}

#[tokio::test]
async fn test_finalize_commitment_fails_before_window_closes() {
    let mut env =
        setup_finalize_commitment_env(LAMPORTS_PER_SOL, LAMPORTS_PER_SOL).await;

    propose(&mut env).await.unwrap();
    approve(&mut env).await.unwrap();

    assert!(finalize(&mut env).await.is_err());
}

#[tokio::test]
async fn test_finalize_commitment_fails_without_approval() {
    let mut env =
        setup_finalize_commitment_env(LAMPORTS_PER_SOL, LAMPORTS_PER_SOL).await;

    propose(&mut env).await.unwrap();
    warp_past_challenge_window(&mut env).await;

    assert!(finalize(&mut env).await.is_err());
}

#[tokio::test]
async fn test_finalize_commitment_fails_with_wrong_operator() {
    let mut env =
        setup_finalize_commitment_env(LAMPORTS_PER_SOL, LAMPORTS_PER_SOL).await;
    let wrong_operator = Keypair::new();
    add_lamport_account_to_context(&mut env.context, wrong_operator.pubkey())
        .await;

    propose(&mut env).await.unwrap();
    approve(&mut env).await.unwrap();
    warp_past_challenge_window(&mut env).await;

    let ix = finalize_commitment(
        wrong_operator.pubkey(),
        env.delegation_cluster,
        env.delegated_account,
        env.commit_id,
    );

    assert!(process_ix(&mut env.context, ix, &[&wrong_operator])
        .await
        .is_err());
}

#[tokio::test]
async fn test_finalize_commitment_fails_with_owner_mismatch() {
    let mut env =
        setup_finalize_commitment_env(LAMPORTS_PER_SOL, LAMPORTS_PER_SOL).await;

    propose(&mut env).await.unwrap();
    let delegation_record =
        delegation_record_pda_from_delegated_account(&env.delegated_account);
    let account = AccountSharedData::from(Account {
        lamports: LAMPORTS_PER_SOL,
        data: create_delegation_record_data(
            env.delegation_cluster,
            Pubkey::new_unique(),
            Some(LAMPORTS_PER_SOL),
        ),
        owner: dpi::ID,
        executable: false,
        rent_epoch: 0,
    });
    env.context.set_account(&delegation_record, &account);
    approve(&mut env).await.unwrap();
    warp_past_challenge_window(&mut env).await;

    assert!(finalize(&mut env).await.is_err());
}

#[tokio::test]
async fn test_finalize_commitment_lamport_increase() {
    let mut env = setup_finalize_commitment_env(
        LAMPORTS_PER_SOL,
        LAMPORTS_PER_SOL + 1_000,
    )
    .await;

    propose(&mut env).await.unwrap();
    approve(&mut env).await.unwrap();
    warp_past_challenge_window(&mut env).await;

    let operator_balance_before =
        balance(&mut env.context, env.operator.pubkey()).await;
    finalize(&mut env).await.unwrap();

    assert_eq!(
        balance(&mut env.context, env.operator.pubkey()).await,
        operator_balance_before - 1_000
    );
    assert_eq!(
        balance(&mut env.context, env.delegated_account).await,
        env.committed_lamports
    );
    assert_eq!(
        read_delegation_record(&mut env).await.lamports,
        env.committed_lamports
    );
}

#[tokio::test]
async fn test_finalize_commitment_lamport_decrease() {
    let mut env = setup_finalize_commitment_env(
        LAMPORTS_PER_SOL,
        LAMPORTS_PER_SOL - 1_000,
    )
    .await;

    propose(&mut env).await.unwrap();
    approve(&mut env).await.unwrap();
    warp_past_challenge_window(&mut env).await;

    let operator_balance_before =
        balance(&mut env.context, env.operator.pubkey()).await;
    finalize(&mut env).await.unwrap();

    assert_eq!(
        balance(&mut env.context, env.operator.pubkey()).await,
        operator_balance_before + 1_000
    );
    assert_eq!(
        balance(&mut env.context, env.delegated_account).await,
        env.committed_lamports
    );
    assert_eq!(
        read_delegation_record(&mut env).await.lamports,
        env.committed_lamports
    );
}

#[tokio::test]
async fn test_finalize_commitment_fails_twice() {
    let mut env =
        setup_finalize_commitment_env(LAMPORTS_PER_SOL, LAMPORTS_PER_SOL).await;

    propose(&mut env).await.unwrap();
    approve(&mut env).await.unwrap();
    warp_past_challenge_window(&mut env).await;
    finalize(&mut env).await.unwrap();

    assert!(finalize(&mut env).await.is_err());
}

struct FinalizeCommitmentEnv {
    context: ProgramTestContext,
    operator: Keypair,
    verifier: Keypair,
    delegation_cluster: Pubkey,
    delegated_account: Pubkey,
    committed_owner: Pubkey,
    committed_lamports: u64,
    commit_id: u64,
    final_state_data: Vec<u8>,
}

async fn setup_finalize_commitment_env(
    record_lamports: u64,
    committed_lamports: u64,
) -> FinalizeCommitmentEnv {
    let mut program_test =
        ProgramTest::new("delegation_program", dpi::ID, None);
    program_test.prefer_bpf(true);

    let authority = Keypair::new();
    let operator = Keypair::new();
    let verifier = Keypair::new();
    let delegated_account = Pubkey::new_unique();
    let cluster_identity = Keypair::new();
    let cluster_args = challenge_cluster_args(operator.pubkey());
    let delegation_cluster =
        dpi::v2::pda::delegation_cluster_pda(&cluster_identity.pubkey());
    let account_owner = Pubkey::new_unique();
    let committed_owner = account_owner;
    let commit_id = 1;
    let final_state_data = vec![9, 8, 7, 6, 5];

    add_lamport_account(&mut program_test, authority.pubkey());
    add_lamport_account(&mut program_test, operator.pubkey());
    add_lamport_account(&mut program_test, verifier.pubkey());

    program_test.add_account(
        delegated_account,
        Account {
            lamports: record_lamports,
            data: vec![1, 2],
            owner: dpi::ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    program_test.add_account(
        delegation_record_pda_from_delegated_account(&delegated_account),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: create_delegation_record_data(
                delegation_cluster,
                account_owner,
                Some(record_lamports),
            ),
            owner: dpi::ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    program_test.add_account(
        delegation_metadata_pda_from_delegated_account(&delegated_account),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: create_delegation_metadata_data(
                authority.pubkey(),
                &[],
                false,
            ),
            owner: dpi::ID,
            executable: false,
            rent_epoch: 0,
        },
    );

    let mut context = program_test.start_with_context().await;
    initialize_delegation_cluster_with_identity(
        &context.banks_client,
        &context.payer,
        &authority,
        &cluster_identity,
        context.last_blockhash,
        cluster_args.clone(),
    )
    .await;

    let challenge_config = cluster_args.challenge_config.unwrap();
    register_operator_for_cluster(
        &mut context,
        &operator,
        &authority,
        delegation_cluster,
        challenge_config.min_operator_bond,
    )
    .await
    .unwrap();
    register_and_add_verifier(
        &mut context,
        &verifier,
        &authority,
        delegation_cluster,
        challenge_config.min_verifier_bond,
    )
    .await
    .unwrap();

    write_state(
        &mut context,
        &operator,
        delegated_account,
        WriteStateBufferArgs {
            commit_id,
            total_len: final_state_data.len() as u32,
            offset: 0,
            chunk: final_state_data.clone(),
        },
    )
    .await
    .unwrap();

    FinalizeCommitmentEnv {
        context,
        operator,
        verifier,
        delegation_cluster,
        delegated_account,
        committed_owner,
        committed_lamports,
        commit_id,
        final_state_data,
    }
}

async fn propose(
    env: &mut FinalizeCommitmentEnv,
) -> Result<(), BanksClientError> {
    let ix = propose_commitment(
        env.operator.pubkey(),
        env.delegation_cluster,
        env.delegated_account,
        ProposeCommitmentArgs {
            commit_id: env.commit_id,
            lamports: env.committed_lamports,
            owner: env.committed_owner,
            da_pointer_hash: [9; 32],
            er_slot: Some(42),
        },
    );

    process_ix(&mut env.context, ix, &[&env.operator]).await
}

async fn approve(
    env: &mut FinalizeCommitmentEnv,
) -> Result<(), BanksClientError> {
    let ix = approve_commitment(
        env.verifier.pubkey(),
        env.delegation_cluster,
        env.delegated_account,
        env.commit_id,
    );

    process_ix(&mut env.context, ix, &[&env.verifier]).await
}

async fn finalize(
    env: &mut FinalizeCommitmentEnv,
) -> Result<(), BanksClientError> {
    let ix = finalize_commitment(
        env.operator.pubkey(),
        env.delegation_cluster,
        env.delegated_account,
        env.commit_id,
    );

    process_ix(&mut env.context, ix, &[&env.operator]).await
}

async fn register_operator_for_cluster(
    context: &mut ProgramTestContext,
    operator: &Keypair,
    authority: &Keypair,
    delegation_cluster: Pubkey,
    stake_lamports: u64,
) -> Result<(), BanksClientError> {
    let ix = register_operator(
        operator.pubkey(),
        authority.pubkey(),
        delegation_cluster,
        RegisterOperatorArgs { stake_lamports },
    );

    process_ix(context, ix, &[operator, authority]).await
}

async fn register_and_add_verifier(
    context: &mut ProgramTestContext,
    verifier: &Keypair,
    authority: &Keypair,
    delegation_cluster: Pubkey,
    stake_lamports: u64,
) -> Result<(), BanksClientError> {
    let ix = register_verifier(
        verifier.pubkey(),
        authority.pubkey(),
        delegation_cluster,
        RegisterVerifierArgs { stake_lamports },
    );
    process_ix(context, ix, &[verifier, authority]).await?;

    let ix = update_verifier_registry(
        authority.pubkey(),
        delegation_cluster,
        verifier.pubkey(),
        UpdateVerifierRegistryArgs {
            action: VerifierRegistryAction::Add.value(),
            weight: 1,
        },
    );

    process_ix(context, ix, &[authority]).await
}

async fn write_state(
    context: &mut ProgramTestContext,
    operator: &Keypair,
    account: Pubkey,
    args: WriteStateBufferArgs,
) -> Result<(), BanksClientError> {
    let ix = write_state_buffer(
        context.payer.pubkey(),
        operator.pubkey(),
        account,
        args,
    );

    process_ix(context, ix, &[operator]).await
}

async fn process_ix(
    context: &mut ProgramTestContext,
    ix: Instruction,
    signers: &[&Keypair],
) -> Result<(), BanksClientError> {
    let latest_blockhash: Hash =
        context.banks_client.get_latest_blockhash().await.unwrap();
    let blockhash = context
        .banks_client
        .get_new_latest_blockhash(&latest_blockhash)
        .await
        .unwrap();
    let tx = {
        let mut all_signers = Vec::with_capacity(signers.len() + 1);
        all_signers.push(&context.payer);
        all_signers.extend_from_slice(signers);

        Transaction::new_signed_with_payer(
            &[ix],
            Some(&context.payer.pubkey()),
            &all_signers,
            blockhash,
        )
    };

    context.banks_client.process_transaction(tx).await
}

async fn warp_past_challenge_window(env: &mut FinalizeCommitmentEnv) {
    let pending_commitment = read_pending_commitment(env).await;
    env.context
        .warp_to_slot(pending_commitment.challenge_window_end_slot + 1)
        .unwrap();
}

struct PendingCommitmentSnapshot {
    status: u8,
    challenge_window_end_slot: u64,
}

async fn read_pending_commitment(
    env: &mut FinalizeCommitmentEnv,
) -> PendingCommitmentSnapshot {
    let pending_commitment_account = env
        .context
        .banks_client
        .get_account(pending_commitment_pda(
            &env.delegated_account,
            env.commit_id,
        ))
        .await
        .unwrap()
        .unwrap();

    let pending_commitment = <PendingCommitment as Decodable>::decode(
        &pending_commitment_account.data,
    )
    .unwrap();

    PendingCommitmentSnapshot {
        status: pending_commitment.status(),
        challenge_window_end_slot: pending_commitment
            .challenge_window_end_slot(),
    }
}

async fn read_delegation_record(
    env: &mut FinalizeCommitmentEnv,
) -> DelegationRecord {
    let account = env
        .context
        .banks_client
        .get_account(delegation_record_pda_from_delegated_account(
            &env.delegated_account,
        ))
        .await
        .unwrap()
        .unwrap();

    *DelegationRecord::try_from_bytes_with_discriminator(&account.data).unwrap()
}

async fn read_delegation_metadata(
    env: &mut FinalizeCommitmentEnv,
) -> DelegationMetadata {
    let account = env
        .context
        .banks_client
        .get_account(delegation_metadata_pda_from_delegated_account(
            &env.delegated_account,
        ))
        .await
        .unwrap()
        .unwrap();

    DelegationMetadata::try_from_bytes_with_discriminator(&account.data)
        .unwrap()
}

async fn balance(context: &mut ProgramTestContext, pubkey: Pubkey) -> u64 {
    context.banks_client.get_balance(pubkey).await.unwrap()
}

fn add_lamport_account(program_test: &mut ProgramTest, pubkey: Pubkey) {
    program_test.add_account(
        pubkey,
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
}

async fn add_lamport_account_to_context(
    context: &mut ProgramTestContext,
    pubkey: Pubkey,
) {
    let ix = solana_system_interface::instruction::transfer(
        &context.payer.pubkey(),
        &pubkey,
        LAMPORTS_PER_SOL,
    );
    let latest_blockhash =
        context.banks_client.get_latest_blockhash().await.unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        latest_blockhash,
    );

    context.banks_client.process_transaction(tx).await.unwrap();
}
