use dlp_api::{
    pda::{
        delegation_metadata_pda_from_delegated_account,
        delegation_record_pda_from_delegated_account, fees_vault_pda,
    },
    v2::{
        instruction_builder::{
            challenger_reveal, post_commitment, raise_challenge,
            register_operator, register_verifier, resolve_dispute,
            update_verifier_registry, write_state_buffer,
        },
        pda::{challenge_pda, operator_bond_pda, pending_commitment_pda},
        Challenge, ChallengerRevealArgs, OperatorBond, PendingCommitment,
        PostCommitmentArgs, RaiseChallengeArgs, RegisterOperatorArgs,
        RegisterVerifierArgs, ResolveDisputeArgs, WriteStateBufferArgs,
        CHALLENGE_OUTCOME_CHALLENGER_CORRECT_OPERATOR_SLASHED,
        CHALLENGE_OUTCOME_OPERATOR_CORRECT_CHALLENGER_SLASHED,
        CHALLENGE_STATUS_AWAITING_RESOLVER, CHALLENGE_STATUS_TERMINAL,
        DISPUTE_DECISION_CHALLENGER_STATE_CORRECT,
        DISPUTE_DECISION_OPERATOR_STATE_CORRECT, OPERATOR_STATUS_SLASHED,
        PENDING_COMMITMENT_STATUS_AWAITING_CHALLENGER_REVEAL,
        PENDING_COMMITMENT_STATUS_AWAITING_DISPUTE_RESOLUTION,
        PENDING_COMMITMENT_STATUS_RESOLVED_CHALLENGER,
        PENDING_COMMITMENT_STATUS_RESOLVED_OPERATOR,
        RESOLVED_STATE_SOURCE_CHALLENGER_REVEAL,
        RESOLVED_STATE_SOURCE_OPERATOR_COMMITMENT,
        VERIFIER_REGISTRY_ACTION_ADD,
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
    v2::{initialize_protocol_config, valid_protocol_config_args},
};

const CHALLENGE_STAKE: u64 = 10_000;
const OPERATOR_STAKE: u64 = 50_000;

#[tokio::test]
async fn test_resolve_dispute_operator_correct_slashes_challenger_and_selects_operator(
) {
    let mut env = setup_resolve_dispute_env().await;
    open_dispute(&mut env).await;

    let fee_vault_before =
        account_lamports(&mut env.context, fees_vault_pda()).await;
    let challenger_before =
        account_lamports(&mut env.context, env.challenger.pubkey()).await;
    let operator_bond_before = read_operator_bond(&mut env).await;

    resolve_v2_dispute(
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

    resolve_v2_dispute(
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
    assert_eq!(operator_bond_after.status, OPERATOR_STATUS_SLASHED);
    assert_eq!(operator_bond_after.stake_lamports, 0);
    assert_eq!(operator_bond_after.locked_lamports, 0);
    assert_eq!(
        operator_bond_after.account_lamports,
        operator_bond_before.account_lamports - OPERATOR_STAKE
    );
}

#[tokio::test]
async fn test_resolve_dispute_fails_with_wrong_resolver() {
    let mut env = setup_resolve_dispute_env().await;
    open_dispute(&mut env).await;

    let ix = resolve_dispute(
        env.operator.pubkey(),
        env.operator.pubkey(),
        env.challenger.pubkey(),
        env.delegated_account,
        env.commit_id,
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
    post_v2_commitment(&mut env).await.unwrap();

    let data_hash = account_data_hash(&[4, 3, 2, 1]);
    let reveal_args = valid_reveal_args(
        env.committed_lamports,
        env.committed_owner,
        data_hash,
    );
    raise_v2_challenge_for_reveal(&mut env, &reveal_args, CHALLENGE_STAKE)
        .await
        .unwrap();

    let pending = read_pending_commitment(&mut env).await;
    assert_eq!(
        pending.status,
        PENDING_COMMITMENT_STATUS_AWAITING_CHALLENGER_REVEAL
    );

    assert!(resolve_v2_dispute(
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
        env.operator.pubkey(),
        env.challenger.pubkey(),
        env.delegated_account,
        env.commit_id,
        ResolveDisputeArgs {
            decision: DISPUTE_DECISION_OPERATOR_STATE_CORRECT,
        },
    );
    ix.accounts[3].pubkey = operator_bond_pda(&env.other_operator.pubkey());

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
        env.operator.pubkey(),
        env.challenger.pubkey(),
        env.delegated_account,
        env.commit_id,
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

    assert!(
        resolve_v2_dispute(&mut env, ResolveDisputeArgs { decision: 99 })
            .await
            .is_err()
    );
}

struct ResolveDisputeEnv {
    context: ProgramTestContext,
    resolver: Keypair,
    operator: Keypair,
    other_operator: Keypair,
    challenger: Keypair,
    delegated_account: Pubkey,
    committed_owner: Pubkey,
    committed_lamports: u64,
    commit_id: u64,
}

async fn setup_resolve_dispute_env() -> ResolveDisputeEnv {
    let mut program_test = ProgramTest::new("dlp", dlp_api::ID, None);
    program_test.prefer_bpf(true);

    let authority = Keypair::new();
    let resolver = Keypair::new();
    let operator = Keypair::new();
    let other_operator = Keypair::new();
    let verifier = Keypair::new();
    let challenger = Keypair::new();
    let delegated_account = Pubkey::new_unique();
    let committed_owner = Pubkey::new_unique();
    let commit_id = 1;
    let operator_state_data = vec![9, 8, 7, 6, 5];
    let record_lamports = LAMPORTS_PER_SOL;
    let committed_lamports = LAMPORTS_PER_SOL;

    add_lamport_account(&mut program_test, authority.pubkey());
    add_lamport_account(&mut program_test, resolver.pubkey());
    add_lamport_account(&mut program_test, operator.pubkey());
    add_lamport_account(&mut program_test, other_operator.pubkey());
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
                operator.pubkey(),
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
    let mut config_args = valid_protocol_config_args();
    config_args.resolver = resolver.pubkey();
    initialize_protocol_config(
        &context.banks_client,
        &context.payer,
        &authority,
        context.last_blockhash,
        config_args.clone(),
    )
    .await;

    register_v2_operator(&mut context, &operator, &authority, OPERATOR_STAKE)
        .await
        .unwrap();
    register_v2_operator(
        &mut context,
        &other_operator,
        &authority,
        OPERATOR_STAKE,
    )
    .await
    .unwrap();
    register_and_add_v2_verifier(
        &mut context,
        &verifier,
        &authority,
        config_args.min_verifier_bond,
    )
    .await
    .unwrap();

    write_v2_state_buffer(
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

    ResolveDisputeEnv {
        context,
        resolver,
        operator,
        other_operator,
        challenger,
        delegated_account,
        committed_owner,
        committed_lamports,
        commit_id,
    }
}

async fn open_dispute(env: &mut ResolveDisputeEnv) -> ChallengerRevealArgs {
    post_v2_commitment(env).await.unwrap();

    let challenger_data = vec![4, 3, 2, 1];
    let reveal_args = valid_reveal_args(
        env.committed_lamports,
        env.committed_owner,
        account_data_hash(&challenger_data),
    );
    write_challenger_state_buffer(env, challenger_data)
        .await
        .unwrap();
    raise_v2_challenge_for_reveal(env, &reveal_args, CHALLENGE_STAKE)
        .await
        .unwrap();
    reveal_v2_challenge(env, reveal_args.clone()).await.unwrap();

    let pending = read_pending_commitment(env).await;
    assert_eq!(
        pending.status,
        PENDING_COMMITMENT_STATUS_AWAITING_DISPUTE_RESOLUTION
    );
    let challenge = read_challenge(env).await;
    assert_eq!(challenge.status, CHALLENGE_STATUS_AWAITING_RESOLVER);

    reveal_args
}

async fn post_v2_commitment(
    env: &mut ResolveDisputeEnv,
) -> Result<(), BanksClientError> {
    let ix = post_commitment(
        env.operator.pubkey(),
        env.delegated_account,
        PostCommitmentArgs {
            commit_id: env.commit_id,
            lamports: env.committed_lamports,
            owner: env.committed_owner,
            da_pointer_hash: [9; 32],
            er_slot: Some(42),
        },
    );

    process_ix(&mut env.context, ix, &[&env.operator]).await
}

async fn raise_v2_challenge_for_reveal(
    env: &mut ResolveDisputeEnv,
    reveal_args: &ChallengerRevealArgs,
    stake_lamports: u64,
) -> Result<(), BanksClientError> {
    let pending = read_pending_commitment(env).await;
    let ix = raise_challenge(
        env.challenger.pubkey(),
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

async fn reveal_v2_challenge(
    env: &mut ResolveDisputeEnv,
    args: ChallengerRevealArgs,
) -> Result<(), BanksClientError> {
    let ix = challenger_reveal(
        env.challenger.pubkey(),
        env.operator.pubkey(),
        env.delegated_account,
        env.commit_id,
        args,
    );

    process_ix(&mut env.context, ix, &[&env.challenger]).await
}

async fn resolve_v2_dispute(
    env: &mut ResolveDisputeEnv,
    args: ResolveDisputeArgs,
) -> Result<(), BanksClientError> {
    let ix = resolve_dispute(
        env.resolver.pubkey(),
        env.operator.pubkey(),
        env.challenger.pubkey(),
        env.delegated_account,
        env.commit_id,
        args,
    );

    process_ix(&mut env.context, ix, &[&env.resolver]).await
}

async fn write_challenger_state_buffer(
    env: &mut ResolveDisputeEnv,
    data: Vec<u8>,
) -> Result<(), BanksClientError> {
    write_v2_state_buffer(
        &mut env.context,
        &env.challenger,
        env.delegated_account,
        WriteStateBufferArgs {
            commit_id: env.commit_id,
            total_len: data.len() as u32,
            offset: 0,
            chunk: data,
        },
    )
    .await
}

async fn register_v2_operator(
    context: &mut ProgramTestContext,
    operator: &Keypair,
    authority: &Keypair,
    amount_lamports: u64,
) -> Result<(), BanksClientError> {
    let ix = register_operator(
        operator.pubkey(),
        authority.pubkey(),
        RegisterOperatorArgs { amount_lamports },
    );

    process_ix(context, ix, &[operator, authority]).await
}

async fn register_and_add_v2_verifier(
    context: &mut ProgramTestContext,
    verifier: &Keypair,
    authority: &Keypair,
    amount_lamports: u64,
) -> Result<(), BanksClientError> {
    let ix = register_verifier(
        verifier.pubkey(),
        authority.pubkey(),
        RegisterVerifierArgs { amount_lamports },
    );
    process_ix(context, ix, &[verifier, authority]).await?;

    let ix = update_verifier_registry(
        authority.pubkey(),
        verifier.pubkey(),
        dlp_api::v2::UpdateVerifierRegistryArgs {
            action: VERIFIER_REGISTRY_ACTION_ADD,
            weight: 1,
        },
    );

    process_ix(context, ix, &[authority]).await
}

async fn write_v2_state_buffer(
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
        .get_account(operator_bond_pda(&env.operator.pubkey()))
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

fn challenge_address(env: &ResolveDisputeEnv) -> Pubkey {
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
