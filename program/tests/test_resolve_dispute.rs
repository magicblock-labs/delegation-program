use delegation_program_interface as dpi;
use dpi::{
    instruction_builder::{
        challenger_reveal, finalize_challenger_commitment, finalize_commitment,
        propose_commitment, raise_challenge, register_operator,
        register_verifier, resolve_dispute, update_delegation_cluster,
        update_verifier_registry, write_state_buffer,
    },
    legacy::pda::{
        delegation_metadata_pda_from_delegated_account,
        delegation_record_pda_from_delegated_account, fees_vault_pda,
    },
    pda::{challenge_pda, operator_bond_pda, pending_commitment_pda},
    Challenge, ChallengeConfig, ChallengerRevealArgs, OperatorBond,
    OperatorStatus, PendingCommitment, ProposeCommitmentArgs,
    RaiseChallengeArgs, RegisterOperatorArgs, RegisterVerifierArgs,
    ResolveDisputeArgs, UpdateDelegationClusterArgs,
    UpdateVerifierRegistryArgs, VerifierRegistryAction, WriteStateBufferArgs,
    CHALLENGE_OUTCOME_CHALLENGER_CORRECT_OPERATOR_SLASHED,
    CHALLENGE_OUTCOME_OPERATOR_CORRECT_CHALLENGER_SLASHED,
    CHALLENGE_STATUS_AWAITING_RESOLVER, CHALLENGE_STATUS_TERMINAL,
    DISPUTE_DECISION_CHALLENGER_STATE_CORRECT,
    DISPUTE_DECISION_OPERATOR_STATE_CORRECT,
    PENDING_COMMITMENT_STATUS_AWAITING_CHALLENGER_REVEAL,
    PENDING_COMMITMENT_STATUS_AWAITING_DISPUTE_RESOLUTION,
    PENDING_COMMITMENT_STATUS_FINALIZED,
    PENDING_COMMITMENT_STATUS_RESOLVED_CHALLENGER,
    PENDING_COMMITMENT_STATUS_RESOLVED_OPERATOR,
    RESOLVED_STATE_SOURCE_CHALLENGER_REVEAL,
    RESOLVED_STATE_SOURCE_OPERATOR_COMMITMENT,
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

const CHALLENGE_STAKE: u64 = 10_000;
const OPERATOR_STAKE: u64 = 50_000;

#[tokio::test]
async fn test_resolve_dispute_operator_correct_slashes_challenger_and_selects_operator(
) {
    let mut env = setup_resolve_dispute_env().await;
    open_dispute(&mut env).await;

    let mut updated_config = env.challenge_config.clone();
    updated_config.resolver = Pubkey::new_unique();
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

    let fee_vault_before =
        account_lamports(&mut env.context, fees_vault_pda()).await;
    let challenger_before =
        account_lamports(&mut env.context, env.challenger.pubkey()).await;
    let operator_bond_before = read_operator_bond(&mut env).await;

    resolve(
        &mut env,
        ResolveDisputeArgs {
            decision: DISPUTE_DECISION_OPERATOR_STATE_CORRECT,
        },
    )
    .await
    .unwrap();

    let challenge = read_challenge(&mut env).await;
    assert_eq!(challenge.status, CHALLENGE_STATUS_TERMINAL);
    assert_eq!(
        challenge.outcome,
        CHALLENGE_OUTCOME_OPERATOR_CORRECT_CHALLENGER_SLASHED
    );
    assert_eq!(
        challenge.account_lamports,
        Rent::default().minimum_balance(Challenge::DATA_LEN)
    );

    let pending = read_pending_commitment(&mut env).await;
    assert_eq!(pending.status, PENDING_COMMITMENT_STATUS_RESOLVED_OPERATOR);
    assert_eq!(pending.active_challenge, None);
    assert_eq!(
        pending.resolved_state_source,
        Some(RESOLVED_STATE_SOURCE_OPERATOR_COMMITMENT)
    );
    assert_eq!(
        account_lamports(&mut env.context, fees_vault_pda()).await,
        fee_vault_before + CHALLENGE_STAKE
    );
    assert_eq!(
        account_lamports(&mut env.context, env.challenger.pubkey()).await,
        challenger_before
    );

    let operator_bond_after = read_operator_bond(&mut env).await;
    assert_eq!(operator_bond_after.status, operator_bond_before.status);
    assert_eq!(
        operator_bond_after.stake_lamports,
        operator_bond_before.stake_lamports
    );
    assert_eq!(
        operator_bond_after.account_lamports,
        operator_bond_before.account_lamports
    );

    let ix = finalize_challenger_commitment(
        env.challenger.pubkey(),
        env.delegation_cluster,
        env.delegated_account,
        env.commit_seq,
    );
    assert!(process_ix(&mut env.context, ix, &[&env.challenger])
        .await
        .is_err());

    let ix = finalize_commitment(
        env.operator.pubkey(),
        env.delegation_cluster,
        env.delegated_account,
        env.commit_seq,
    );
    process_ix(&mut env.context, ix, &[&env.operator])
        .await
        .unwrap();

    assert_eq!(
        read_pending_commitment(&mut env).await.status,
        PENDING_COMMITMENT_STATUS_FINALIZED
    );
    assert_eq!(
        env.context
            .banks_client
            .get_account(env.delegated_account)
            .await
            .unwrap()
            .unwrap()
            .data,
        env.operator_state_data
    );
}

#[tokio::test]
async fn test_resolve_dispute_challenger_correct_refunds_challenger_and_slashes_operator(
) {
    let mut env = setup_resolve_dispute_env().await;
    open_dispute(&mut env).await;

    let fee_vault_before =
        account_lamports(&mut env.context, fees_vault_pda()).await;
    let challenger_before =
        account_lamports(&mut env.context, env.challenger.pubkey()).await;
    let operator_bond_before = read_operator_bond(&mut env).await;

    resolve(
        &mut env,
        ResolveDisputeArgs {
            decision: DISPUTE_DECISION_CHALLENGER_STATE_CORRECT,
        },
    )
    .await
    .unwrap();

    let challenge = read_challenge(&mut env).await;
    assert_eq!(challenge.status, CHALLENGE_STATUS_TERMINAL);
    assert_eq!(
        challenge.outcome,
        CHALLENGE_OUTCOME_CHALLENGER_CORRECT_OPERATOR_SLASHED
    );
    assert_eq!(
        challenge.account_lamports,
        Rent::default().minimum_balance(Challenge::DATA_LEN)
    );

    let pending = read_pending_commitment(&mut env).await;
    assert_eq!(
        pending.status,
        PENDING_COMMITMENT_STATUS_RESOLVED_CHALLENGER
    );
    assert_eq!(pending.active_challenge, None);
    assert_eq!(
        pending.resolved_state_source,
        Some(RESOLVED_STATE_SOURCE_CHALLENGER_REVEAL)
    );
    assert_eq!(
        account_lamports(&mut env.context, env.challenger.pubkey()).await,
        challenger_before + CHALLENGE_STAKE
    );
    assert_eq!(
        account_lamports(&mut env.context, fees_vault_pda()).await,
        fee_vault_before + OPERATOR_STAKE
    );

    let operator_bond_after = read_operator_bond(&mut env).await;
    assert_eq!(operator_bond_after.status, OperatorStatus::Slashed.value());
    assert_eq!(operator_bond_after.stake_lamports, 0);
    assert_eq!(operator_bond_after.locked_lamports, 0);
    assert_eq!(
        operator_bond_after.account_lamports,
        operator_bond_before.account_lamports - OPERATOR_STAKE
    );

    let ix = finalize_commitment(
        env.operator.pubkey(),
        env.delegation_cluster,
        env.delegated_account,
        env.commit_seq,
    );
    assert!(process_ix(&mut env.context, ix, &[&env.operator])
        .await
        .is_err());

    let ix = finalize_challenger_commitment(
        env.challenger.pubkey(),
        env.delegation_cluster,
        env.delegated_account,
        env.commit_seq,
    );
    process_ix(&mut env.context, ix, &[&env.challenger])
        .await
        .unwrap();

    assert_eq!(
        read_pending_commitment(&mut env).await.status,
        PENDING_COMMITMENT_STATUS_FINALIZED
    );
    assert_eq!(
        env.context
            .banks_client
            .get_account(env.delegated_account)
            .await
            .unwrap()
            .unwrap()
            .data,
        env.challenger_state_data
    );
}

#[tokio::test]
async fn test_resolve_dispute_fails_with_wrong_resolver() {
    let mut env = setup_resolve_dispute_env().await;
    open_dispute(&mut env).await;

    let ix = resolve_dispute(
        env.operator.pubkey(),
        env.delegation_cluster,
        env.challenger.pubkey(),
        env.delegated_account,
        env.commit_seq,
        ResolveDisputeArgs {
            decision: DISPUTE_DECISION_OPERATOR_STATE_CORRECT,
        },
    );

    assert!(process_ix(&mut env.context, ix, &[&env.operator])
        .await
        .is_err());
}

#[tokio::test]
async fn test_resolve_dispute_fails_before_mismatched_reveal() {
    let mut env = setup_resolve_dispute_env().await;
    propose(&mut env).await.unwrap();

    let data_hash = account_data_hash(&[4, 3, 2, 1]);
    let reveal_args = valid_reveal_args(
        env.committed_lamports,
        env.committed_owner,
        data_hash,
    );
    raise_for_reveal(&mut env, &reveal_args, CHALLENGE_STAKE)
        .await
        .unwrap();

    let pending = read_pending_commitment(&mut env).await;
    assert_eq!(
        pending.status,
        PENDING_COMMITMENT_STATUS_AWAITING_CHALLENGER_REVEAL
    );

    assert!(resolve(
        &mut env,
        ResolveDisputeArgs {
            decision: DISPUTE_DECISION_OPERATOR_STATE_CORRECT,
        },
    )
    .await
    .is_err());
}

#[tokio::test]
async fn test_resolve_dispute_fails_with_wrong_operator_bond() {
    let mut env = setup_resolve_dispute_env().await;
    open_dispute(&mut env).await;

    let mut ix = resolve_dispute(
        env.resolver.pubkey(),
        env.delegation_cluster,
        env.challenger.pubkey(),
        env.delegated_account,
        env.commit_seq,
        ResolveDisputeArgs {
            decision: DISPUTE_DECISION_OPERATOR_STATE_CORRECT,
        },
    );
    ix.accounts[3].pubkey = challenge_address(&env);

    assert!(process_ix(&mut env.context, ix, &[&env.resolver])
        .await
        .is_err());
}

#[tokio::test]
async fn test_resolve_dispute_fails_with_wrong_fee_vault() {
    let mut env = setup_resolve_dispute_env().await;
    open_dispute(&mut env).await;

    let mut ix = resolve_dispute(
        env.resolver.pubkey(),
        env.delegation_cluster,
        env.challenger.pubkey(),
        env.delegated_account,
        env.commit_seq,
        ResolveDisputeArgs {
            decision: DISPUTE_DECISION_OPERATOR_STATE_CORRECT,
        },
    );
    ix.accounts[6].pubkey = env.challenger.pubkey();

    assert!(process_ix(&mut env.context, ix, &[&env.resolver])
        .await
        .is_err());
}

#[tokio::test]
async fn test_resolve_dispute_fails_with_invalid_decision() {
    let mut env = setup_resolve_dispute_env().await;
    open_dispute(&mut env).await;

    assert!(resolve(&mut env, ResolveDisputeArgs { decision: 99 })
        .await
        .is_err());
}

struct ResolveDisputeEnv {
    context: ProgramTestContext,
    config_authority: Keypair,
    resolver: Keypair,
    operator: Keypair,
    challenger: Keypair,
    delegation_cluster: Pubkey,
    delegated_account: Pubkey,
    committed_owner: Pubkey,
    committed_lamports: u64,
    commit_seq: u32,
    challenge_config: ChallengeConfig,
    operator_state_data: Vec<u8>,
    challenger_state_data: Vec<u8>,
}

async fn setup_resolve_dispute_env() -> ResolveDisputeEnv {
    let mut program_test =
        ProgramTest::new("delegation_program", dpi::ID, None);
    program_test.prefer_bpf(true);

    let authority = Keypair::new();
    let resolver = Keypair::new();
    let operator = Keypair::new();
    let verifier = Keypair::new();
    let challenger = Keypair::new();
    let delegated_account = Pubkey::new_unique();
    let cluster_identity = Keypair::new();
    let mut cluster_args = challenge_cluster_args(operator.pubkey());
    let challenge_config = cluster_args.challenge_config.as_mut().unwrap();
    challenge_config.resolver = resolver.pubkey();
    challenge_config.min_operator_bond = OPERATOR_STAKE;
    let delegation_cluster =
        dpi::pda::delegation_cluster_pda(&cluster_identity.pubkey());
    let challenge_config = cluster_args.challenge_config.clone().unwrap();
    let committed_owner = Pubkey::new_unique();
    let commit_seq = 1;
    let operator_state_data = vec![9, 8, 7, 6, 5];
    let challenger_state_data = vec![4, 3, 2, 1];
    let record_lamports = LAMPORTS_PER_SOL;
    let committed_lamports = LAMPORTS_PER_SOL;

    add_lamport_account(&mut program_test, authority.pubkey());
    add_lamport_account(&mut program_test, resolver.pubkey());
    add_lamport_account(&mut program_test, operator.pubkey());
    add_lamport_account(&mut program_test, verifier.pubkey());
    add_lamport_account(&mut program_test, challenger.pubkey());
    add_protocol_fee_vault(&mut program_test);

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

    register_operator_for_cluster(
        &mut context,
        &operator,
        &authority,
        delegation_cluster,
        OPERATOR_STAKE,
    )
    .await
    .unwrap();
    register_and_add_verifier(
        &mut context,
        &verifier,
        &authority,
        delegation_cluster,
        cluster_args
            .challenge_config
            .as_ref()
            .unwrap()
            .min_verifier_bond,
    )
    .await
    .unwrap();

    write_state(
        &mut context,
        &operator,
        delegated_account,
        WriteStateBufferArgs {
            commit_seq,
            total_len: operator_state_data.len() as u32,
            offset: 0,
            chunk: operator_state_data.clone(),
        },
    )
    .await
    .unwrap();

    ResolveDisputeEnv {
        context,
        config_authority: authority,
        resolver,
        operator,
        challenger,
        delegation_cluster,
        delegated_account,
        committed_owner,
        committed_lamports,
        commit_seq,
        challenge_config,
        operator_state_data,
        challenger_state_data,
    }
}

async fn open_dispute(env: &mut ResolveDisputeEnv) {
    propose(env).await.unwrap();

    let reveal_args = valid_reveal_args(
        env.committed_lamports,
        env.committed_owner,
        account_data_hash(&env.challenger_state_data),
    );
    write_challenger_state_buffer(env, env.challenger_state_data.clone())
        .await
        .unwrap();
    raise_for_reveal(env, &reveal_args, CHALLENGE_STAKE)
        .await
        .unwrap();
    reveal(env, reveal_args.clone()).await.unwrap();

    let pending = read_pending_commitment(env).await;
    assert_eq!(
        pending.status,
        PENDING_COMMITMENT_STATUS_AWAITING_DISPUTE_RESOLUTION
    );
    let challenge = read_challenge(env).await;
    assert_eq!(challenge.status, CHALLENGE_STATUS_AWAITING_RESOLVER);
}

async fn propose(env: &mut ResolveDisputeEnv) -> Result<(), BanksClientError> {
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

async fn raise_for_reveal(
    env: &mut ResolveDisputeEnv,
    reveal_args: &ChallengerRevealArgs,
    stake_lamports: u64,
) -> Result<(), BanksClientError> {
    let pending = read_pending_commitment(env).await;
    let ix = raise_challenge(
        env.challenger.pubkey(),
        env.delegation_cluster,
        env.delegated_account,
        env.commit_seq,
        RaiseChallengeArgs {
            state_commitment_hash: pending.state_commitment_hash,
            challenge_hash: challenge_hash(
                pending.state_commitment_hash,
                env.operator.pubkey(),
                env.challenger.pubkey(),
                env.delegated_account,
                env.commit_seq,
                reveal_args,
            ),
            stake_lamports,
        },
    );

    process_ix(&mut env.context, ix, &[&env.challenger]).await
}

async fn reveal(
    env: &mut ResolveDisputeEnv,
    args: ChallengerRevealArgs,
) -> Result<(), BanksClientError> {
    let ix = challenger_reveal(
        env.challenger.pubkey(),
        env.operator.pubkey(),
        env.delegation_cluster,
        env.delegated_account,
        env.commit_seq,
        args,
    );

    process_ix(&mut env.context, ix, &[&env.challenger]).await
}

async fn resolve(
    env: &mut ResolveDisputeEnv,
    args: ResolveDisputeArgs,
) -> Result<(), BanksClientError> {
    let ix = resolve_dispute(
        env.resolver.pubkey(),
        env.delegation_cluster,
        env.challenger.pubkey(),
        env.delegated_account,
        env.commit_seq,
        args,
    );

    process_ix(&mut env.context, ix, &[&env.resolver]).await
}

async fn write_challenger_state_buffer(
    env: &mut ResolveDisputeEnv,
    data: Vec<u8>,
) -> Result<(), BanksClientError> {
    write_state(
        &mut env.context,
        &env.challenger,
        env.delegated_account,
        WriteStateBufferArgs {
            commit_seq: env.commit_seq,
            total_len: data.len() as u32,
            offset: 0,
            chunk: data,
        },
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
    env: &mut ResolveDisputeEnv,
) -> PendingCommitmentSnapshot {
    let account = env
        .context
        .banks_client
        .get_account(pending_commitment_pda(
            &env.delegated_account,
            env.commit_seq,
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
    account_lamports: u64,
}

async fn read_challenge(env: &mut ResolveDisputeEnv) -> ChallengeSnapshot {
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
        account_lamports: account.lamports,
    }
}

struct OperatorBondSnapshot {
    status: u8,
    stake_lamports: u64,
    locked_lamports: u64,
    account_lamports: u64,
}

async fn read_operator_bond(
    env: &mut ResolveDisputeEnv,
) -> OperatorBondSnapshot {
    let account = env
        .context
        .banks_client
        .get_account(operator_bond_pda(&env.delegation_cluster))
        .await
        .unwrap()
        .unwrap();

    let operator_bond =
        <OperatorBond as Decodable>::decode(&account.data).unwrap();

    OperatorBondSnapshot {
        status: operator_bond.status(),
        stake_lamports: operator_bond.stake_lamports(),
        locked_lamports: operator_bond.locked_lamports(),
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
            owner: dpi::ID,
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

fn challenge_address(env: &ResolveDisputeEnv) -> Pubkey {
    challenge_pda(
        &env.delegated_account,
        env.commit_seq,
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
    commit_seq: u32,
    args: &ChallengerRevealArgs,
) -> [u8; 32] {
    solana_sha256_hasher::hashv(&[
        b"magicblock.challenge.v1",
        &state_commitment_hash,
        operator.as_ref(),
        challenger.as_ref(),
        account.as_ref(),
        &commit_seq.to_le_bytes(),
        &args.lamports.to_le_bytes(),
        args.owner.as_ref(),
        &args.data_hash,
        &args.salt,
    ])
    .to_bytes()
}
