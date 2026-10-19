use dlp_api::{
    pda::delegation_record_pda_from_delegated_account,
    v2::{
        instruction_builder::{
            register_operator, register_verifier, submit_commitment,
            update_verifier_registry, write_state_buffer,
        },
        pda::{pending_commitment_pda, verifier_registry_pda},
        ChallengeConfig, PendingCommitment, RegisterOperatorArgs,
        RegisterVerifierArgs, SubmitCommitmentArgs, UpdateVerifierRegistryArgs,
        VerifierRegistry, VerifierRegistryAction, WriteStateBufferArgs,
        PENDING_COMMITMENT_STATUS_ACTIVE,
    },
};
use solana_program::native_token::LAMPORTS_PER_SOL;
use solana_program_test::{
    BanksClient, ProgramTest, ProgramTestBanksClientExt,
};
use solana_sdk::{
    account::Account,
    hash::Hash,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use solana_sdk_ids::system_program;
use wheels::layout::Decodable;

mod fixtures;

use crate::fixtures::{
    create_delegation_record_data,
    v2::{challenge_cluster_args, initialize_delegation_cluster},
};

#[tokio::test]
async fn test_submit_commitment() {
    let mut env = setup_submit_commitment_env(2, true, false).await;
    let args = submit_commitment_args();

    submit(&mut env, args.clone()).await.unwrap();

    let pending_commitment_account = env
        .banks
        .get_account(pending_commitment_pda(
            &env.delegated_account,
            args.commit_id,
        ))
        .await
        .unwrap()
        .unwrap();
    let pending_commitment = <PendingCommitment as Decodable>::decode(
        &pending_commitment_account.data,
    )
    .unwrap();

    assert_eq!(
        pending_commitment.discriminator(),
        PendingCommitment::DISCRIMINATOR
    );
    assert_eq!(
        pending_commitment.status(),
        PENDING_COMMITMENT_STATUS_ACTIVE
    );
    assert_eq!(
        *pending_commitment.delegation_cluster(),
        env.delegation_cluster
    );
    assert_eq!(
        *pending_commitment.operator_identity(),
        env.operator.pubkey()
    );
    assert_eq!(*pending_commitment.account_pubkey(), env.delegated_account);
    assert_eq!(pending_commitment.commit_id(), args.commit_id);
    assert_eq!(pending_commitment.lamports(), args.lamports);
    assert_eq!(*pending_commitment.owner(), args.owner);
    assert_eq!(
        *pending_commitment.data_hash(),
        account_data_hash(&state_data())
    );
    assert_eq!(*pending_commitment.da_pointer_hash(), args.da_pointer_hash);
    assert_eq!(pending_commitment.approval_count(), 0);
    assert_eq!(
        pending_commitment.approval_threshold(),
        env.challenge_config.approval_threshold
    );
    assert_eq!(
        *pending_commitment.resolver(),
        env.challenge_config.resolver
    );
    assert_eq!(
        pending_commitment.min_challenger_stake(),
        env.challenge_config.min_challenger_stake
    );
    assert_eq!(
        pending_commitment.challenger_reveal_timeout_slots(),
        env.challenge_config.challenger_reveal_timeout_slots
    );
    assert_eq!(
        pending_commitment.match_penalty_bps(),
        env.challenge_config.match_penalty_bps
    );
    assert_eq!(
        pending_commitment.challenge_window_end_slot(),
        pending_commitment.posted_slot()
            + env.challenge_config.challenge_window_slots
    );

    let selected: Vec<_> = pending_commitment
        .selected_verifiers()
        .iter()
        .map(|entry| (*entry.verifier_identity(), entry.approved()))
        .collect();
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0], (env.verifiers[0].pubkey(), false));
    assert!(selected
        .iter()
        .all(|(verifier, _)| *verifier != env.operator.pubkey()));

    let verifier_registry_account = env
        .banks
        .get_account(verifier_registry_pda(&env.delegation_cluster))
        .await
        .unwrap()
        .unwrap();
    let verifier_registry = <VerifierRegistry as Decodable>::decode(
        &verifier_registry_account.data,
    )
    .unwrap();

    assert_eq!(verifier_registry.next_selection_index(), 2);
}

#[tokio::test]
async fn test_submit_commitment_fails_twice() {
    let mut env = setup_submit_commitment_env(2, true, false).await;
    let args = submit_commitment_args();

    submit(&mut env, args.clone()).await.unwrap();

    assert!(submit(&mut env, args).await.is_err());
}

#[tokio::test]
async fn test_submit_commitment_fails_without_validator_signature() {
    let env = setup_submit_commitment_env(2, false, false).await;
    let args = submit_commitment_args();
    let mut ix = submit_commitment(
        env.operator.pubkey(),
        env.delegation_cluster,
        env.delegated_account,
        args,
    );
    ix.accounts[0].is_signer = false;

    let blockhash = env.banks.get_latest_blockhash().await.unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&env.payer.pubkey()),
        &[&env.payer],
        blockhash,
    );

    assert!(env.banks.process_transaction(tx).await.is_err());
}

#[tokio::test]
async fn test_submit_commitment_fails_without_enough_verifiers() {
    let mut env = setup_submit_commitment_env(0, true, false).await;

    assert!(submit(&mut env, submit_commitment_args()).await.is_err());
}

#[tokio::test]
async fn test_submit_commitment_fails_without_state_buffer() {
    let mut env = setup_submit_commitment_env(2, true, false).await;
    let mut args = submit_commitment_args();
    args.commit_id = 99;

    assert!(submit(&mut env, args).await.is_err());
}

#[tokio::test]
async fn test_submit_commitment_fails_with_wrong_delegation_authority() {
    let mut env = setup_submit_commitment_env(2, true, true).await;

    assert!(submit(&mut env, submit_commitment_args()).await.is_err());
}

struct SubmitCommitmentEnv {
    banks: BanksClient,
    payer: Keypair,
    operator: Keypair,
    verifiers: Vec<Keypair>,
    delegation_cluster: Pubkey,
    delegated_account: Pubkey,
    challenge_config: ChallengeConfig,
}

async fn setup_submit_commitment_env(
    verifier_count: usize,
    register_operator_as_verifier: bool,
    wrong_delegation_authority: bool,
) -> SubmitCommitmentEnv {
    let mut program_test = ProgramTest::new("dlp", dlp_api::ID, None);
    program_test.prefer_bpf(true);

    let authority = Keypair::new();
    let operator = Keypair::new();
    let verifiers = (0..verifier_count)
        .map(|_| Keypair::new())
        .collect::<Vec<_>>();
    let delegated_account = Pubkey::new_unique();
    let cluster_args = challenge_cluster_args(operator.pubkey());
    let delegation_cluster =
        dlp_api::v2::pda::delegation_cluster_pda(&cluster_args.cluster_id);
    let delegation_authority = if wrong_delegation_authority {
        Pubkey::new_unique()
    } else {
        delegation_cluster
    };

    add_lamport_account(&mut program_test, authority.pubkey());
    add_lamport_account(&mut program_test, operator.pubkey());
    for verifier in verifiers.iter() {
        add_lamport_account(&mut program_test, verifier.pubkey());
    }

    program_test.add_account(
        delegated_account,
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: state_data(),
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
                delegation_authority,
                Pubkey::new_unique(),
                Some(LAMPORTS_PER_SOL),
            ),
            owner: dlp_api::ID,
            executable: false,
            rent_epoch: 0,
        },
    );

    let (mut banks, payer, blockhash) = program_test.start().await;
    initialize_delegation_cluster(
        &banks,
        &payer,
        &authority,
        blockhash,
        cluster_args.clone(),
    )
    .await;
    let challenge_config = cluster_args.challenge_config.unwrap();

    register_operator_for_cluster(
        &banks,
        &payer,
        &operator,
        &authority,
        delegation_cluster,
        challenge_config.min_operator_bond,
    )
    .await;

    if register_operator_as_verifier {
        register_and_add_verifier(
            &banks,
            &payer,
            &operator,
            &authority,
            delegation_cluster,
            challenge_config.min_verifier_bond,
        )
        .await;
    }

    for verifier in verifiers.iter() {
        register_and_add_verifier(
            &banks,
            &payer,
            verifier,
            &authority,
            delegation_cluster,
            challenge_config.min_verifier_bond,
        )
        .await;
    }

    write_state(
        &mut banks,
        &payer,
        &operator,
        delegated_account,
        WriteStateBufferArgs {
            commit_id: submit_commitment_args().commit_id,
            total_len: state_data().len() as u32,
            offset: 0,
            chunk: state_data(),
        },
    )
    .await;

    SubmitCommitmentEnv {
        banks,
        payer,
        operator,
        verifiers,
        delegation_cluster,
        delegated_account,
        challenge_config,
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

async fn register_operator_for_cluster(
    banks: &BanksClient,
    payer: &Keypair,
    operator: &Keypair,
    authority: &Keypair,
    delegation_cluster: Pubkey,
    stake_lamports: u64,
) {
    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let ix = register_operator(
        operator.pubkey(),
        authority.pubkey(),
        delegation_cluster,
        RegisterOperatorArgs { stake_lamports },
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer, operator, authority],
        blockhash,
    );

    banks.process_transaction(tx).await.unwrap();
}

async fn register_and_add_verifier(
    banks: &BanksClient,
    payer: &Keypair,
    verifier: &Keypair,
    authority: &Keypair,
    delegation_cluster: Pubkey,
    stake_lamports: u64,
) {
    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let ix = register_verifier(
        verifier.pubkey(),
        authority.pubkey(),
        delegation_cluster,
        RegisterVerifierArgs { stake_lamports },
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer, verifier, authority],
        blockhash,
    );
    banks.process_transaction(tx).await.unwrap();

    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let ix = update_verifier_registry(
        authority.pubkey(),
        delegation_cluster,
        verifier.pubkey(),
        UpdateVerifierRegistryArgs {
            action: VerifierRegistryAction::Add.value(),
            weight: 1,
        },
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer, authority],
        blockhash,
    );

    banks.process_transaction(tx).await.unwrap();
}

fn submit_commitment_args() -> SubmitCommitmentArgs {
    SubmitCommitmentArgs {
        commit_id: 1,
        lamports: 1_000,
        owner: Pubkey::new_unique(),
        da_pointer_hash: [9; 32],
        er_slot: Some(42),
    }
}

async fn write_state(
    banks: &mut BanksClient,
    payer: &Keypair,
    operator: &Keypair,
    account: Pubkey,
    args: WriteStateBufferArgs,
) {
    let latest_blockhash: Hash = banks.get_latest_blockhash().await.unwrap();
    let blockhash = banks
        .get_new_latest_blockhash(&latest_blockhash)
        .await
        .unwrap();
    let ix =
        write_state_buffer(payer.pubkey(), operator.pubkey(), account, args);
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer, operator],
        blockhash,
    );

    banks.process_transaction(tx).await.unwrap();
}

fn state_data() -> Vec<u8> {
    vec![1, 2, 3, 4]
}

fn account_data_hash(data: &[u8]) -> [u8; 32] {
    solana_sha256_hasher::hashv(&[b"magicblock.account_data.v1", data])
        .to_bytes()
}

async fn submit(
    env: &mut SubmitCommitmentEnv,
    args: SubmitCommitmentArgs,
) -> Result<(), solana_program_test::BanksClientError> {
    let latest_blockhash: Hash =
        env.banks.get_latest_blockhash().await.unwrap();
    let blockhash = env
        .banks
        .get_new_latest_blockhash(&latest_blockhash)
        .await
        .unwrap();
    let ix = submit_commitment(
        env.operator.pubkey(),
        env.delegation_cluster,
        env.delegated_account,
        args,
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&env.payer.pubkey()),
        &[&env.payer, &env.operator],
        blockhash,
    );

    env.banks.process_transaction(tx).await
}
