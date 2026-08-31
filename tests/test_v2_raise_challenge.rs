use dlp_api::{
    pda::{
        delegation_metadata_pda_from_delegated_account,
        delegation_record_pda_from_delegated_account,
    },
    v2::{
        instruction_builder::{
            approve_commitment, finalize_commitment, post_commitment,
            raise_challenge, register_operator, register_verifier,
            update_verifier_registry, write_state_buffer,
        },
        pda::{challenge_pda, pending_commitment_pda, CHALLENGE_SEED},
        Challenge, PendingCommitment, PostCommitmentArgs, RaiseChallengeArgs,
        RegisterOperatorArgs, RegisterVerifierArgs, WriteStateBufferArgs,
        CHALLENGE_OUTCOME_NONE,
        CHALLENGE_STATUS_AWAITING_REVEAL,
        PENDING_COMMITMENT_STATUS_AWAITING_CHALLENGER_REVEAL,
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
    v2::{init_v2, valid_args},
};

#[test]
fn test_v2_challenge_pda_uses_account_commit_id_and_challenger() {
    let account = Pubkey::new_unique();
    let challenger = Pubkey::new_unique();
    let commit_id = 7_u64;
    let expected = Pubkey::find_program_address(
        &[
            CHALLENGE_SEED,
            account.as_ref(),
            &commit_id.to_le_bytes(),
            challenger.as_ref(),
        ],
        &dlp_api::ID,
    )
    .0;

    assert_eq!(challenge_pda(&account, commit_id, &challenger), expected);
}

#[tokio::test]
async fn test_v2_raise_challenge() {
    let mut env = setup_raise_challenge_env().await;
    post_v2_commitment(&mut env).await.unwrap();

    let pending_before = read_pending_commitment(&mut env).await;
    let args = valid_raise_challenge_args(pending_before.state_commitment_hash);
    let challenge_address = challenge_pda(
        &env.delegated_account,
        env.commit_id,
        &env.challenger.pubkey(),
    );

    raise_v2_challenge(&mut env, args.clone()).await.unwrap();

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
        pending_commitment_pda(&env.delegated_account, env.commit_id)
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
            + env.config_args.challenger_reveal_timeout_slots
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
async fn test_v2_raise_challenge_fails_without_challenger_signature() {
    let mut env = setup_raise_challenge_env().await;
    post_v2_commitment(&mut env).await.unwrap();

    let pending = read_pending_commitment(&mut env).await;
    let args = valid_raise_challenge_args(pending.state_commitment_hash);
    let mut ix = raise_challenge(
        env.challenger.pubkey(),
        env.delegated_account,
        env.commit_id,
        args,
    );
    ix.accounts[0].is_signer = false;

    assert!(process_ix(&mut env.context, ix, &[]).await.is_err());
}

#[tokio::test]
async fn test_v2_raise_challenge_fails_below_min_stake() {
    let mut env = setup_raise_challenge_env().await;
    post_v2_commitment(&mut env).await.unwrap();

    let pending = read_pending_commitment(&mut env).await;
    let mut args = valid_raise_challenge_args(pending.state_commitment_hash);
    args.stake_lamports = env.config_args.min_challenger_stake - 1;

    assert!(raise_v2_challenge(&mut env, args).await.is_err());
}

#[tokio::test]
async fn test_v2_raise_challenge_fails_after_challenge_window() {
    let mut env = setup_raise_challenge_env().await;
    post_v2_commitment(&mut env).await.unwrap();
    warp_past_challenge_window(&mut env).await;

    let pending = read_pending_commitment(&mut env).await;
    let args = valid_raise_challenge_args(pending.state_commitment_hash);

    assert!(raise_v2_challenge(&mut env, args).await.is_err());
}

#[tokio::test]
async fn test_v2_raise_challenge_fails_with_wrong_state_commitment_hash() {
    let mut env = setup_raise_challenge_env().await;
    post_v2_commitment(&mut env).await.unwrap();

    let mut args = valid_raise_challenge_args([1; 32]);
    args.state_commitment_hash[0] ^= 1;

    assert!(raise_v2_challenge(&mut env, args).await.is_err());
}

#[tokio::test]
async fn test_v2_raise_challenge_fails_when_challenge_already_active() {
    let mut env = setup_raise_challenge_env().await;
    post_v2_commitment(&mut env).await.unwrap();

    let pending = read_pending_commitment(&mut env).await;
    let args = valid_raise_challenge_args(pending.state_commitment_hash);
    raise_v2_challenge(&mut env, args.clone()).await.unwrap();

    assert!(raise_v2_challenge(&mut env, args).await.is_err());
}

#[tokio::test]
async fn test_v2_finalize_commitment_fails_with_active_challenge() {
    let mut env = setup_raise_challenge_env().await;
    post_v2_commitment(&mut env).await.unwrap();
    approve_v2_commitment(&mut env).await.unwrap();

    let pending = read_pending_commitment(&mut env).await;
    let args = valid_raise_challenge_args(pending.state_commitment_hash);
    raise_v2_challenge(&mut env, args).await.unwrap();
    warp_past_challenge_window(&mut env).await;

    assert!(finalize_v2_commitment(&mut env).await.is_err());
}

struct RaiseChallengeEnv {
    context: ProgramTestContext,
    operator: Keypair,
    verifier: Keypair,
    challenger: Keypair,
    delegated_account: Pubkey,
    committed_owner: Pubkey,
    committed_lamports: u64,
    commit_id: u64,
    config_args: dlp_api::v2::InitProtocolConfigArgs,
}

async fn setup_raise_challenge_env() -> RaiseChallengeEnv {
    let mut program_test = ProgramTest::new("dlp", dlp_api::ID, None);
    program_test.prefer_bpf(true);

    let authority = Keypair::new();
    let operator = Keypair::new();
    let verifier = Keypair::new();
    let challenger = Keypair::new();
    let delegated_account = Pubkey::new_unique();
    let committed_owner = Pubkey::new_unique();
    let commit_id = 1;
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
    let config_args = valid_args();
    init_v2(
        &context.banks_client,
        &context.payer,
        &authority,
        context.last_blockhash,
        config_args.clone(),
    )
    .await;

    register_v2_operator(
        &mut context,
        &operator,
        &authority,
        config_args.min_operator_bond,
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
            total_len: final_state_data.len() as u32,
            offset: 0,
            chunk: final_state_data.clone(),
        },
    )
    .await
    .unwrap();

    RaiseChallengeEnv {
        context,
        operator,
        verifier,
        challenger,
        delegated_account,
        committed_owner,
        committed_lamports,
        commit_id,
        config_args,
    }
}

async fn post_v2_commitment(
    env: &mut RaiseChallengeEnv,
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

async fn approve_v2_commitment(
    env: &mut RaiseChallengeEnv,
) -> Result<(), BanksClientError> {
    let ix = approve_commitment(
        env.verifier.pubkey(),
        env.delegated_account,
        env.commit_id,
    );

    process_ix(&mut env.context, ix, &[&env.verifier]).await
}

async fn raise_v2_challenge(
    env: &mut RaiseChallengeEnv,
    args: RaiseChallengeArgs,
) -> Result<(), BanksClientError> {
    let ix = raise_challenge(
        env.challenger.pubkey(),
        env.delegated_account,
        env.commit_id,
        args,
    );

    process_ix(&mut env.context, ix, &[&env.challenger]).await
}

async fn finalize_v2_commitment(
    env: &mut RaiseChallengeEnv,
) -> Result<(), BanksClientError> {
    let ix = finalize_commitment(
        env.operator.pubkey(),
        env.delegated_account,
        env.commit_id,
    );

    process_ix(&mut env.context, ix, &[&env.operator]).await
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
