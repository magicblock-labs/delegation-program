use delegation_program_interface as dpi;
use dpi::{
    instruction_builder::{
        approve_commitment, finalize_commitment, propose_commitment,
        raise_challenge, register_operator, register_verifier,
        update_delegation_cluster, update_verifier_registry,
        write_state_buffer,
    },
    legacy::pda::{
        delegation_metadata_pda_from_delegated_account,
        delegation_record_pda_from_delegated_account,
    },
    pda::{challenge_pda, pending_commitment_pda},
    Challenge, ChallengeConfig, PendingCommitment, ProposeCommitmentArgs,
    RaiseChallengeArgs, RegisterOperatorArgs, RegisterVerifierArgs,
    UpdateDelegationClusterArgs, UpdateVerifierRegistryArgs,
    VerifierRegistryAction, WriteStateBufferArgs, CHALLENGE_OUTCOME_NONE,
    CHALLENGE_STATUS_AWAITING_REVEAL,
    PENDING_COMMITMENT_STATUS_AWAITING_CHALLENGER_REVEAL,
};
use solana_program::{native_token::LAMPORTS_PER_SOL, rent::Rent};
use solana_program_test::{
    BanksClientError, ProgramTest, ProgramTestBanksClientExt,
    ProgramTestContext,
};
use solana_sdk::{
    account::Account,
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
async fn test_raise_challenge() {
    let mut env = setup_raise_challenge_env().await;
    propose(&mut env).await.unwrap();

    let pending_before = read_pending_commitment(&mut env).await;
    let args = valid_raise_challenge_args(pending_before.state_commitment_hash);
    let challenge_address = challenge_pda(
        &env.delegated_account,
        env.commit_seq,
        &env.challenger.pubkey(),
    );

    raise(&mut env, args.clone()).await.unwrap();

    let challenge_account = env
        .context
        .banks_client
        .get_account(challenge_address)
        .await
        .unwrap()
        .unwrap();
    let challenge =
        <Challenge as Decodable>::decode(&challenge_account.data).unwrap();

    assert_eq!(challenge.discriminator(), Challenge::DISCRIMINATOR);
    assert_eq!(challenge.status(), CHALLENGE_STATUS_AWAITING_REVEAL);
    assert_eq!(challenge.outcome(), CHALLENGE_OUTCOME_NONE);
    assert_eq!(
        *challenge.pending_commitment(),
        pending_commitment_pda(&env.delegated_account, env.commit_seq)
    );
    assert_eq!(*challenge.challenger_identity(), env.challenger.pubkey());
    assert_eq!(
        *challenge.state_commitment_hash(),
        args.state_commitment_hash
    );
    assert_eq!(*challenge.challenge_hash(), args.challenge_hash);
    assert_eq!(challenge.challenger_lamports(), 0);
    assert_eq!(*challenge.challenger_owner(), Pubkey::default());
    assert_eq!(*challenge.challenger_data_hash(), [0; 32]);
    assert_eq!(*challenge.challenger_state_buffer(), Pubkey::default());
    assert_eq!(challenge.challenger_stake_lamports(), args.stake_lamports);
    assert_eq!(
        challenge.reveal_deadline_slot(),
        challenge.raised_slot()
            + env.challenge_config.challenger_reveal_timeout_slots
    );
    assert_eq!(
        challenge_account.lamports,
        Rent::default().minimum_balance(Challenge::DATA_LEN)
            + args.stake_lamports
    );

    let pending_after = read_pending_commitment(&mut env).await;
    assert_eq!(
        pending_after.status,
        PENDING_COMMITMENT_STATUS_AWAITING_CHALLENGER_REVEAL
    );
    assert_eq!(pending_after.active_challenge, Some(challenge_address));
}

#[tokio::test]
async fn test_raise_challenge_fails_without_challenger_signature() {
    let mut env = setup_raise_challenge_env().await;
    propose(&mut env).await.unwrap();

    let pending = read_pending_commitment(&mut env).await;
    let args = valid_raise_challenge_args(pending.state_commitment_hash);
    let mut ix = raise_challenge(
        env.challenger.pubkey(),
        env.delegation_cluster,
        env.delegated_account,
        env.commit_seq,
        args,
    );
    ix.accounts[0].is_signer = false;

    assert!(process_ix(&mut env.context, ix, &[]).await.is_err());
}

#[tokio::test]
async fn test_raise_challenge_fails_below_min_stake() {
    let mut env = setup_raise_challenge_env().await;
    propose(&mut env).await.unwrap();

    let pending = read_pending_commitment(&mut env).await;
    let mut args = valid_raise_challenge_args(pending.state_commitment_hash);
    args.stake_lamports = env.challenge_config.min_challenger_stake - 1;

    assert!(raise(&mut env, args).await.is_err());
}

#[tokio::test]
async fn test_raise_challenge_uses_submitted_policy() {
    let mut env = setup_raise_challenge_env().await;
    propose(&mut env).await.unwrap();

    let submitted_config = env.challenge_config.clone();
    let mut updated_config = submitted_config.clone();
    updated_config.min_challenger_stake += 100;
    updated_config.challenger_reveal_timeout_slots += 100;
    let ix = update_delegation_cluster(
        env.config_authority.pubkey(),
        env.delegation_cluster,
        UpdateDelegationClusterArgs {
            challenge_config: Some(updated_config),
        },
    );
    process_ix(&mut env.context, ix, &[&env.config_authority])
        .await
        .unwrap();

    let pending = read_pending_commitment(&mut env).await;
    let mut args = valid_raise_challenge_args(pending.state_commitment_hash);
    args.stake_lamports = submitted_config.min_challenger_stake;
    raise(&mut env, args).await.unwrap();

    let challenge_account = env
        .context
        .banks_client
        .get_account(challenge_pda(
            &env.delegated_account,
            env.commit_seq,
            &env.challenger.pubkey(),
        ))
        .await
        .unwrap()
        .unwrap();
    let challenge =
        <Challenge as Decodable>::decode(&challenge_account.data).unwrap();
    assert_eq!(
        challenge.reveal_deadline_slot(),
        challenge.raised_slot()
            + submitted_config.challenger_reveal_timeout_slots
    );
}

#[tokio::test]
async fn test_raise_challenge_fails_after_challenge_window() {
    let mut env = setup_raise_challenge_env().await;
    propose(&mut env).await.unwrap();
    warp_past_challenge_window(&mut env).await;

    let pending = read_pending_commitment(&mut env).await;
    let args = valid_raise_challenge_args(pending.state_commitment_hash);

    assert!(raise(&mut env, args).await.is_err());
}

#[tokio::test]
async fn test_raise_challenge_fails_with_wrong_state_commitment_hash() {
    let mut env = setup_raise_challenge_env().await;
    propose(&mut env).await.unwrap();

    let mut args = valid_raise_challenge_args([1; 32]);
    args.state_commitment_hash[0] ^= 1;

    assert!(raise(&mut env, args).await.is_err());
}

#[tokio::test]
async fn test_raise_challenge_fails_when_challenge_already_active() {
    let mut env = setup_raise_challenge_env().await;
    propose(&mut env).await.unwrap();

    let pending = read_pending_commitment(&mut env).await;
    let args = valid_raise_challenge_args(pending.state_commitment_hash);
    raise(&mut env, args.clone()).await.unwrap();

    assert!(raise(&mut env, args).await.is_err());
}

#[tokio::test]
async fn test_finalize_commitment_fails_with_active_challenge() {
    let mut env = setup_raise_challenge_env().await;
    propose(&mut env).await.unwrap();
    approve(&mut env).await.unwrap();

    let pending = read_pending_commitment(&mut env).await;
    let args = valid_raise_challenge_args(pending.state_commitment_hash);
    raise(&mut env, args).await.unwrap();
    warp_past_challenge_window(&mut env).await;

    assert!(finalize(&mut env).await.is_err());
}

struct RaiseChallengeEnv {
    context: ProgramTestContext,
    config_authority: Keypair,
    operator: Keypair,
    verifier: Keypair,
    challenger: Keypair,
    delegation_cluster: Pubkey,
    delegated_account: Pubkey,
    committed_owner: Pubkey,
    committed_lamports: u64,
    commit_seq: u64,
    challenge_config: ChallengeConfig,
}

async fn setup_raise_challenge_env() -> RaiseChallengeEnv {
    let mut program_test =
        ProgramTest::new("delegation_program", dpi::ID, None);
    program_test.prefer_bpf(true);

    let authority = Keypair::new();
    let operator = Keypair::new();
    let verifier = Keypair::new();
    let challenger = Keypair::new();
    let delegated_account = Pubkey::new_unique();
    let cluster_identity = Keypair::new();
    let cluster_args = challenge_cluster_args(operator.pubkey());
    let delegation_cluster =
        dpi::pda::delegation_cluster_pda(&cluster_identity.pubkey());
    let committed_owner = Pubkey::new_unique();
    let commit_seq = 1;
    let final_state_data = vec![9, 8, 7, 6, 5];
    let record_lamports = LAMPORTS_PER_SOL;
    let committed_lamports = LAMPORTS_PER_SOL;

    add_lamport_account(&mut program_test, authority.pubkey());
    add_lamport_account(&mut program_test, operator.pubkey());
    add_lamport_account(&mut program_test, verifier.pubkey());
    add_lamport_account(&mut program_test, challenger.pubkey());

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
                committed_owner,
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
            commit_seq,
            total_len: final_state_data.len() as u32,
            offset: 0,
            chunk: final_state_data.clone(),
        },
    )
    .await
    .unwrap();

    RaiseChallengeEnv {
        context,
        config_authority: authority,
        operator,
        verifier,
        challenger,
        delegation_cluster,
        delegated_account,
        committed_owner,
        committed_lamports,
        commit_seq,
        challenge_config,
    }
}

async fn propose(env: &mut RaiseChallengeEnv) -> Result<(), BanksClientError> {
    let ix = propose_commitment(
        env.operator.pubkey(),
        env.delegation_cluster,
        env.delegated_account,
        ProposeCommitmentArgs {
            commit_seq: env.commit_seq,
            lamports: env.committed_lamports,
            owner: env.committed_owner,
            da_pointer_hash: [9; 32],
            er_slot: Some(42),
        },
    );

    process_ix(&mut env.context, ix, &[&env.operator]).await
}

async fn approve(env: &mut RaiseChallengeEnv) -> Result<(), BanksClientError> {
    let ix = approve_commitment(
        env.verifier.pubkey(),
        env.delegation_cluster,
        env.delegated_account,
        env.commit_seq,
    );

    process_ix(&mut env.context, ix, &[&env.verifier]).await
}

async fn raise(
    env: &mut RaiseChallengeEnv,
    args: RaiseChallengeArgs,
) -> Result<(), BanksClientError> {
    let ix = raise_challenge(
        env.challenger.pubkey(),
        env.delegation_cluster,
        env.delegated_account,
        env.commit_seq,
        args,
    );

    process_ix(&mut env.context, ix, &[&env.challenger]).await
}

async fn finalize(env: &mut RaiseChallengeEnv) -> Result<(), BanksClientError> {
    let ix = finalize_commitment(
        env.operator.pubkey(),
        env.delegation_cluster,
        env.delegated_account,
        env.commit_seq,
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

async fn warp_past_challenge_window(env: &mut RaiseChallengeEnv) {
    let pending_commitment = read_pending_commitment(env).await;
    env.context
        .warp_to_slot(pending_commitment.challenge_window_end_slot + 1)
        .unwrap();
}

struct PendingCommitmentSnapshot {
    status: u8,
    active_challenge: Option<Pubkey>,
    state_commitment_hash: [u8; 32],
    challenge_window_end_slot: u64,
}

async fn read_pending_commitment(
    env: &mut RaiseChallengeEnv,
) -> PendingCommitmentSnapshot {
    let pending_commitment_account = env
        .context
        .banks_client
        .get_account(pending_commitment_pda(
            &env.delegated_account,
            env.commit_seq,
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
        active_challenge: pending_commitment.active_challenge().cloned(),
        state_commitment_hash: *pending_commitment.state_commitment_hash(),
        challenge_window_end_slot: pending_commitment
            .challenge_window_end_slot(),
    }
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

fn valid_raise_challenge_args(
    state_commitment_hash: [u8; 32],
) -> RaiseChallengeArgs {
    RaiseChallengeArgs {
        state_commitment_hash,
        challenge_hash: [7; 32],
        stake_lamports: 3,
    }
}
