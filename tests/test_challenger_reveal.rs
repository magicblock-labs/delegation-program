use dlp_api::{
    pda::{
        delegation_metadata_pda_from_delegated_account,
        delegation_record_pda_from_delegated_account, fees_vault_pda,
    },
    v2::{
        instruction_builder::{
            challenger_reveal, raise_challenge, register_operator,
            register_verifier, submit_commitment, update_delegation_cluster,
            update_verifier_registry, write_state_buffer,
        },
        pda::{challenge_pda, pending_commitment_pda, state_buffer_pda},
        Challenge, ChallengeConfig, ChallengerRevealArgs, PendingCommitment,
        RaiseChallengeArgs, RegisterOperatorArgs, RegisterVerifierArgs,
        SubmitCommitmentArgs, UpdateDelegationClusterArgs,
        UpdateVerifierRegistryArgs, VerifierRegistryAction,
        WriteStateBufferArgs, CHALLENGE_OUTCOME_INVALID_REVEAL,
        CHALLENGE_OUTCOME_MATCHING_STATE_CHALLENGER_PENALIZED,
        CHALLENGE_OUTCOME_NONE, CHALLENGE_STATUS_AWAITING_RESOLVER,
        CHALLENGE_STATUS_TERMINAL, PENDING_COMMITMENT_STATUS_ACTIVE,
        PENDING_COMMITMENT_STATUS_AWAITING_DISPUTE_RESOLUTION,
    },
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
    v2::{challenge_cluster_args, initialize_delegation_cluster},
};

const CHALLENGE_STAKE: u64 = 10_000;

#[tokio::test]
async fn test_challenger_reveal_matching_state_penalizes_and_reopens_commitment(
) {
    let mut env = setup_challenger_reveal_env().await;
    submit(&mut env).await.unwrap();

    let submitted_match_penalty_bps = env.challenge_config.match_penalty_bps;
    let mut updated_config = env.challenge_config.clone();
    updated_config.match_penalty_bps += 100;
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

    let data_hash = account_data_hash(&env.operator_state_data);
    let reveal_args = valid_reveal_args(
        env.committed_lamports,
        env.committed_owner,
        data_hash,
    );
    let challenger_data = env.operator_state_data.clone();
    write_challenger_state_buffer(&mut env, challenger_data)
        .await
        .unwrap();

    let challenger_before =
        account_lamports(&mut env.context, env.challenger.pubkey()).await;
    let fee_vault_before =
        account_lamports(&mut env.context, fees_vault_pda()).await;

    raise_for_reveal(&mut env, &reveal_args, CHALLENGE_STAKE)
        .await
        .unwrap();
    reveal(&mut env, reveal_args.clone()).await.unwrap();

    let penalty =
        CHALLENGE_STAKE * u64::from(submitted_match_penalty_bps) / 10_000;
    let challenge_rent = Rent::default().minimum_balance(Challenge::DATA_LEN);
    let challenge = read_challenge(&mut env).await;
    assert_eq!(challenge.status, CHALLENGE_STATUS_TERMINAL);
    assert_eq!(
        challenge.outcome,
        CHALLENGE_OUTCOME_MATCHING_STATE_CHALLENGER_PENALIZED
    );
    assert_eq!(challenge.challenger_lamports, reveal_args.lamports);
    assert_eq!(challenge.challenger_owner, reveal_args.owner);
    assert_eq!(challenge.challenger_data_hash, reveal_args.data_hash);
    assert_eq!(
        challenge.challenger_state_buffer,
        state_buffer_pda(
            &env.delegated_account,
            env.commit_id,
            &env.challenger.pubkey()
        )
    );
    assert_eq!(challenge.account_lamports, challenge_rent);

    let pending = read_pending_commitment(&mut env).await;
    assert_eq!(pending.status, PENDING_COMMITMENT_STATUS_ACTIVE);
    assert_eq!(pending.active_challenge, None);
    assert_eq!(pending.resolved_state_source, None);
    assert_eq!(
        account_lamports(&mut env.context, env.challenger.pubkey()).await,
        challenger_before - challenge_rent - penalty
    );
    assert_eq!(
        account_lamports(&mut env.context, fees_vault_pda()).await,
        fee_vault_before + penalty
    );
}

#[tokio::test]
async fn test_challenger_reveal_mismatch_moves_to_resolver() {
    let mut env = setup_challenger_reveal_env().await;
    submit(&mut env).await.unwrap();

    let challenger_data = vec![4, 3, 2, 1];
    let reveal_args = valid_reveal_args(
        env.committed_lamports,
        env.committed_owner,
        account_data_hash(&challenger_data),
    );
    write_challenger_state_buffer(&mut env, challenger_data)
        .await
        .unwrap();

    let challenger_before =
        account_lamports(&mut env.context, env.challenger.pubkey()).await;
    let fee_vault_before =
        account_lamports(&mut env.context, fees_vault_pda()).await;

    raise_for_reveal(&mut env, &reveal_args, CHALLENGE_STAKE)
        .await
        .unwrap();
    reveal(&mut env, reveal_args.clone()).await.unwrap();

    let challenge_rent = Rent::default().minimum_balance(Challenge::DATA_LEN);
    let challenge_address = challenge_address(&env);
    let challenge = read_challenge(&mut env).await;
    assert_eq!(challenge.status, CHALLENGE_STATUS_AWAITING_RESOLVER);
    assert_eq!(challenge.outcome, CHALLENGE_OUTCOME_NONE);
    assert_eq!(challenge.challenger_lamports, reveal_args.lamports);
    assert_eq!(challenge.challenger_owner, reveal_args.owner);
    assert_eq!(challenge.challenger_data_hash, reveal_args.data_hash);
    assert_eq!(challenge.account_lamports, challenge_rent + CHALLENGE_STAKE);

    let pending = read_pending_commitment(&mut env).await;
    assert_eq!(
        pending.status,
        PENDING_COMMITMENT_STATUS_AWAITING_DISPUTE_RESOLUTION
    );
    assert_eq!(pending.active_challenge, Some(challenge_address));
    assert_eq!(pending.resolved_state_source, None);
    assert_eq!(
        account_lamports(&mut env.context, env.challenger.pubkey()).await,
        challenger_before - challenge_rent - CHALLENGE_STAKE
    );
    assert_eq!(
        account_lamports(&mut env.context, fees_vault_pda()).await,
        fee_vault_before
    );
}

#[tokio::test]
async fn test_challenger_reveal_invalid_hash_slashes_and_reopens_commitment() {
    let mut env = setup_challenger_reveal_env().await;
    submit(&mut env).await.unwrap();

    let challenger_data = vec![4, 3, 2, 1];
    let raise_reveal_args = valid_reveal_args(
        env.committed_lamports,
        env.committed_owner,
        account_data_hash(&challenger_data),
    );
    let mut actual_reveal_args = raise_reveal_args.clone();
    actual_reveal_args.salt[0] ^= 1;
    write_challenger_state_buffer(&mut env, challenger_data)
        .await
        .unwrap();

    let challenger_before =
        account_lamports(&mut env.context, env.challenger.pubkey()).await;
    let fee_vault_before =
        account_lamports(&mut env.context, fees_vault_pda()).await;

    raise_for_reveal(&mut env, &raise_reveal_args, CHALLENGE_STAKE)
        .await
        .unwrap();
    reveal(&mut env, actual_reveal_args.clone()).await.unwrap();

    let challenge_rent = Rent::default().minimum_balance(Challenge::DATA_LEN);
    let challenge = read_challenge(&mut env).await;
    assert_eq!(challenge.status, CHALLENGE_STATUS_TERMINAL);
    assert_eq!(challenge.outcome, CHALLENGE_OUTCOME_INVALID_REVEAL);
    assert_eq!(challenge.challenger_lamports, actual_reveal_args.lamports);
    assert_eq!(challenge.challenger_owner, actual_reveal_args.owner);
    assert_eq!(challenge.challenger_data_hash, actual_reveal_args.data_hash);
    assert_eq!(challenge.account_lamports, challenge_rent);

    let pending = read_pending_commitment(&mut env).await;
    assert_eq!(pending.status, PENDING_COMMITMENT_STATUS_ACTIVE);
    assert_eq!(pending.active_challenge, None);
    assert_eq!(pending.resolved_state_source, None);
    assert_eq!(
        account_lamports(&mut env.context, env.challenger.pubkey()).await,
        challenger_before - challenge_rent - CHALLENGE_STAKE
    );
    assert_eq!(
        account_lamports(&mut env.context, fees_vault_pda()).await,
        fee_vault_before + CHALLENGE_STAKE
    );
}

#[tokio::test]
async fn test_challenger_reveal_fails_after_reveal_deadline() {
    let mut env = setup_challenger_reveal_env().await;
    submit(&mut env).await.unwrap();

    let data_hash = account_data_hash(&env.operator_state_data);
    let reveal_args = valid_reveal_args(
        env.committed_lamports,
        env.committed_owner,
        data_hash,
    );
    let challenger_data = env.operator_state_data.clone();
    write_challenger_state_buffer(&mut env, challenger_data)
        .await
        .unwrap();
    raise_for_reveal(&mut env, &reveal_args, CHALLENGE_STAKE)
        .await
        .unwrap();

    let challenge = read_challenge(&mut env).await;
    env.context
        .warp_to_slot(challenge.reveal_deadline_slot + 1)
        .unwrap();

    assert!(reveal(&mut env, reveal_args).await.is_err());
}

#[tokio::test]
async fn test_challenger_reveal_fails_with_wrong_challenger_buffer() {
    let mut env = setup_challenger_reveal_env().await;
    submit(&mut env).await.unwrap();

    let data_hash = account_data_hash(&env.operator_state_data);
    let reveal_args = valid_reveal_args(
        env.committed_lamports,
        env.committed_owner,
        data_hash,
    );
    raise_for_reveal(&mut env, &reveal_args, CHALLENGE_STAKE)
        .await
        .unwrap();

    let mut ix = challenger_reveal(
        env.challenger.pubkey(),
        env.operator.pubkey(),
        env.delegation_cluster,
        env.delegated_account,
        env.commit_id,
        reveal_args,
    );
    ix.accounts[4].pubkey = state_buffer_pda(
        &env.delegated_account,
        env.commit_id,
        &env.operator.pubkey(),
    );

    assert!(process_ix(&mut env.context, ix, &[&env.challenger])
        .await
        .is_err());
}

#[tokio::test]
async fn test_challenger_reveal_fails_with_unfinalized_challenger_buffer() {
    let mut env = setup_challenger_reveal_env().await;
    submit(&mut env).await.unwrap();

    let expected_data = [1, 2, 3, 4];
    let reveal_args = valid_reveal_args(
        env.committed_lamports,
        env.committed_owner,
        account_data_hash(&expected_data),
    );
    let commit_id = env.commit_id;
    write_challenger_state_buffer_args(
        &mut env,
        WriteStateBufferArgs {
            commit_id,
            total_len: expected_data.len() as u32,
            offset: 0,
            chunk: expected_data[..2].to_vec(),
        },
    )
    .await
    .unwrap();
    raise_for_reveal(&mut env, &reveal_args, CHALLENGE_STAKE)
        .await
        .unwrap();

    assert!(reveal(&mut env, reveal_args).await.is_err());
}

#[tokio::test]
async fn test_challenger_reveal_fails_with_wrong_fee_vault() {
    let mut env = setup_challenger_reveal_env().await;
    submit(&mut env).await.unwrap();

    let data_hash = account_data_hash(&env.operator_state_data);
    let reveal_args = valid_reveal_args(
        env.committed_lamports,
        env.committed_owner,
        data_hash,
    );
    let challenger_data = env.operator_state_data.clone();
    write_challenger_state_buffer(&mut env, challenger_data)
        .await
        .unwrap();
    raise_for_reveal(&mut env, &reveal_args, CHALLENGE_STAKE)
        .await
        .unwrap();

    let mut ix = challenger_reveal(
        env.challenger.pubkey(),
        env.operator.pubkey(),
        env.delegation_cluster,
        env.delegated_account,
        env.commit_id,
        reveal_args,
    );
    ix.accounts[6].pubkey = env.challenger.pubkey();

    assert!(process_ix(&mut env.context, ix, &[&env.challenger])
        .await
        .is_err());
}

struct ChallengerRevealEnv {
    context: ProgramTestContext,
    config_authority: Keypair,
    operator: Keypair,
    challenger: Keypair,
    delegation_cluster: Pubkey,
    delegated_account: Pubkey,
    committed_owner: Pubkey,
    committed_lamports: u64,
    commit_id: u64,
    challenge_config: ChallengeConfig,
    operator_state_data: Vec<u8>,
}

async fn setup_challenger_reveal_env() -> ChallengerRevealEnv {
    let mut program_test = ProgramTest::new("dlp", dlp_api::ID, None);
    program_test.prefer_bpf(true);

    let authority = Keypair::new();
    let operator = Keypair::new();
    let verifier = Keypair::new();
    let challenger = Keypair::new();
    let delegated_account = Pubkey::new_unique();
    let cluster_args = challenge_cluster_args(operator.pubkey());
    let delegation_cluster =
        dlp_api::v2::pda::delegation_cluster_pda(&cluster_args.cluster_id);
    let committed_owner = Pubkey::new_unique();
    let commit_id = 1;
    let operator_state_data = vec![9, 8, 7, 6, 5];
    let record_lamports = LAMPORTS_PER_SOL;
    let committed_lamports = LAMPORTS_PER_SOL;

    add_lamport_account(&mut program_test, authority.pubkey());
    add_lamport_account(&mut program_test, operator.pubkey());
    add_lamport_account(&mut program_test, verifier.pubkey());
    add_lamport_account(&mut program_test, challenger.pubkey());
    add_protocol_fee_vault(&mut program_test);

    program_test.add_account(
        delegated_account,
        Account {
            lamports: record_lamports,
            data: vec![1, 2],
            owner: dlp_api::ID,
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
            owner: dlp_api::ID,
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
            owner: dlp_api::ID,
            executable: false,
            rent_epoch: 0,
        },
    );

    let mut context = program_test.start_with_context().await;
    initialize_delegation_cluster(
        &context.banks_client,
        &context.payer,
        &authority,
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
            total_len: operator_state_data.len() as u32,
            offset: 0,
            chunk: operator_state_data.clone(),
        },
    )
    .await
    .unwrap();

    ChallengerRevealEnv {
        context,
        config_authority: authority,
        operator,
        challenger,
        delegation_cluster,
        delegated_account,
        committed_owner,
        committed_lamports,
        commit_id,
        challenge_config,
        operator_state_data,
    }
}

async fn submit(env: &mut ChallengerRevealEnv) -> Result<(), BanksClientError> {
    let ix = submit_commitment(
        env.operator.pubkey(),
        env.delegation_cluster,
        env.delegated_account,
        SubmitCommitmentArgs {
            commit_id: env.commit_id,
            lamports: env.committed_lamports,
            owner: env.committed_owner,
            da_pointer_hash: [9; 32],
            er_slot: Some(42),
        },
    );

    process_ix(&mut env.context, ix, &[&env.operator]).await
}

async fn raise_for_reveal(
    env: &mut ChallengerRevealEnv,
    reveal_args: &ChallengerRevealArgs,
    stake_lamports: u64,
) -> Result<(), BanksClientError> {
    let pending = read_pending_commitment(env).await;
    let ix = raise_challenge(
        env.challenger.pubkey(),
        env.delegation_cluster,
        env.delegated_account,
        env.commit_id,
        RaiseChallengeArgs {
            state_commitment_hash: pending.state_commitment_hash,
            challenge_hash: challenge_hash(
                pending.state_commitment_hash,
                env.operator.pubkey(),
                env.challenger.pubkey(),
                env.delegated_account,
                env.commit_id,
                reveal_args,
            ),
            stake_lamports,
        },
    );

    process_ix(&mut env.context, ix, &[&env.challenger]).await
}

async fn reveal(
    env: &mut ChallengerRevealEnv,
    args: ChallengerRevealArgs,
) -> Result<(), BanksClientError> {
    let ix = challenger_reveal(
        env.challenger.pubkey(),
        env.operator.pubkey(),
        env.delegation_cluster,
        env.delegated_account,
        env.commit_id,
        args,
    );

    process_ix(&mut env.context, ix, &[&env.challenger]).await
}

async fn write_challenger_state_buffer(
    env: &mut ChallengerRevealEnv,
    data: Vec<u8>,
) -> Result<(), BanksClientError> {
    write_challenger_state_buffer_args(
        env,
        WriteStateBufferArgs {
            commit_id: env.commit_id,
            total_len: data.len() as u32,
            offset: 0,
            chunk: data,
        },
    )
    .await
}

async fn write_challenger_state_buffer_args(
    env: &mut ChallengerRevealEnv,
    args: WriteStateBufferArgs,
) -> Result<(), BanksClientError> {
    write_state(
        &mut env.context,
        &env.challenger,
        env.delegated_account,
        args,
    )
    .await
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
    authority: &Keypair,
    account: Pubkey,
    args: WriteStateBufferArgs,
) -> Result<(), BanksClientError> {
    let ix = write_state_buffer(
        context.payer.pubkey(),
        authority.pubkey(),
        account,
        args,
    );

    process_ix(context, ix, &[authority]).await
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

struct PendingCommitmentSnapshot {
    status: u8,
    active_challenge: Option<Pubkey>,
    resolved_state_source: Option<u8>,
    state_commitment_hash: [u8; 32],
}

async fn read_pending_commitment(
    env: &mut ChallengerRevealEnv,
) -> PendingCommitmentSnapshot {
    let account = env
        .context
        .banks_client
        .get_account(pending_commitment_pda(
            &env.delegated_account,
            env.commit_id,
        ))
        .await
        .unwrap()
        .unwrap();

    let pending =
        <PendingCommitment as Decodable>::decode(&account.data).unwrap();

    PendingCommitmentSnapshot {
        status: pending.status(),
        active_challenge: pending.active_challenge().cloned(),
        resolved_state_source: pending.resolved_state_source(),
        state_commitment_hash: *pending.state_commitment_hash(),
    }
}

struct ChallengeSnapshot {
    status: u8,
    outcome: u8,
    challenger_lamports: u64,
    challenger_owner: Pubkey,
    challenger_data_hash: [u8; 32],
    challenger_state_buffer: Pubkey,
    reveal_deadline_slot: u64,
    account_lamports: u64,
}

async fn read_challenge(env: &mut ChallengerRevealEnv) -> ChallengeSnapshot {
    let account = env
        .context
        .banks_client
        .get_account(challenge_address(env))
        .await
        .unwrap()
        .unwrap();

    let challenge = <Challenge as Decodable>::decode(&account.data).unwrap();

    ChallengeSnapshot {
        status: challenge.status(),
        outcome: challenge.outcome(),
        challenger_lamports: challenge.challenger_lamports(),
        challenger_owner: *challenge.challenger_owner(),
        challenger_data_hash: *challenge.challenger_data_hash(),
        challenger_state_buffer: *challenge.challenger_state_buffer(),
        reveal_deadline_slot: challenge.reveal_deadline_slot(),
        account_lamports: account.lamports,
    }
}

async fn account_lamports(
    context: &mut ProgramTestContext,
    pubkey: Pubkey,
) -> u64 {
    context
        .banks_client
        .get_account(pubkey)
        .await
        .unwrap()
        .unwrap()
        .lamports
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

fn add_protocol_fee_vault(program_test: &mut ProgramTest) {
    program_test.add_account(
        fees_vault_pda(),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![0; 8],
            owner: dlp_api::ID,
            executable: false,
            rent_epoch: 0,
        },
    );
}

fn valid_reveal_args(
    lamports: u64,
    owner: Pubkey,
    data_hash: [u8; 32],
) -> ChallengerRevealArgs {
    ChallengerRevealArgs {
        lamports,
        owner,
        data_hash,
        salt: [7; 32],
    }
}

fn challenge_address(env: &ChallengerRevealEnv) -> Pubkey {
    challenge_pda(
        &env.delegated_account,
        env.commit_id,
        &env.challenger.pubkey(),
    )
}

fn account_data_hash(data: &[u8]) -> [u8; 32] {
    solana_sha256_hasher::hashv(&[b"magicblock.account_data.v1", data])
        .to_bytes()
}

fn challenge_hash(
    state_commitment_hash: [u8; 32],
    operator: Pubkey,
    challenger: Pubkey,
    account: Pubkey,
    commit_id: u64,
    args: &ChallengerRevealArgs,
) -> [u8; 32] {
    solana_sha256_hasher::hashv(&[
        b"magicblock.challenge.v1",
        &state_commitment_hash,
        operator.as_ref(),
        challenger.as_ref(),
        account.as_ref(),
        &commit_id.to_le_bytes(),
        &args.lamports.to_le_bytes(),
        args.owner.as_ref(),
        &args.data_hash,
        &args.salt,
    ])
    .to_bytes()
}
