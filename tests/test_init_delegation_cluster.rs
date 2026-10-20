use dlp::solana_program;
use dlp_api::{
    pda::fees_vault_pda,
    v2::{
        instruction_builder::init_delegation_cluster,
        pda::{
            delegation_cluster_pda, verifier_registry_pda,
            DELEGATION_CLUSTER_SEED, VERIFIER_REGISTRY_SEED,
        },
        ChallengeConfig, DelegationCluster, InitDelegationClusterArgs,
        VerifierRegistry,
    },
};
use solana_program::{hash::Hash, native_token::LAMPORTS_PER_SOL};
use solana_program_test::{BanksClient, ProgramTest};
use solana_sdk::{
    account::Account,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use solana_sdk_ids::system_program;
use wheels::layout::Decodable;

mod fixtures;

#[tokio::test]
async fn test_init_challenge_delegation_cluster() {
    let (banks, payer, config_authority, cluster_identity, blockhash) =
        setup_program_test_env().await;
    let args = challenge_cluster_args();
    let delegation_cluster = delegation_cluster_pda(&cluster_identity.pubkey());
    let verifier_registry = verifier_registry_pda(&delegation_cluster);
    let ix = init_delegation_cluster(
        config_authority.pubkey(),
        cluster_identity.pubkey(),
        args.clone(),
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &config_authority, &cluster_identity],
        blockhash,
    );

    banks.process_transaction(tx).await.unwrap();

    let delegation_cluster_account = banks
        .get_account(delegation_cluster)
        .await
        .unwrap()
        .unwrap();
    let cluster =
        DelegationCluster::decode(&delegation_cluster_account.data).unwrap();
    let challenge_config = cluster.challenge_config().unwrap();
    let (_, expected_cluster_bump) = Pubkey::find_program_address(
        &[DELEGATION_CLUSTER_SEED, cluster_identity.pubkey().as_ref()],
        &dlp_api::id(),
    );

    assert_eq!(
        delegation_cluster_account.data.len(),
        DelegationCluster::MAX_DATA_LEN
    );
    assert_eq!(cluster.discriminator(), DelegationCluster::DISCRIMINATOR);
    assert_eq!(cluster.bump(), expected_cluster_bump);
    assert_eq!(*cluster.cluster_id(), cluster_identity.pubkey());
    assert_eq!(*cluster.config_authority(), config_authority.pubkey());
    assert_eq!(
        *cluster.validator_rotation_authority(),
        args.validator_rotation_authority
    );
    assert_eq!(*cluster.current_validator(), args.current_validator);
    assert!(!cluster.paused());
    assert_eq!(*cluster.protocol_fee_vault(), fees_vault_pda());

    let expected_config = args.challenge_config.as_ref().unwrap();
    assert_eq!(*challenge_config.resolver(), expected_config.resolver);
    assert_eq!(
        challenge_config.min_operator_bond(),
        expected_config.min_operator_bond
    );
    assert_eq!(
        challenge_config.min_verifier_bond(),
        expected_config.min_verifier_bond
    );
    assert_eq!(
        challenge_config.min_challenger_stake(),
        expected_config.min_challenger_stake
    );
    assert_eq!(
        challenge_config.challenge_window_slots(),
        expected_config.challenge_window_slots
    );
    assert_eq!(
        challenge_config.operator_response_timeout_slots(),
        expected_config.operator_response_timeout_slots
    );
    assert_eq!(
        challenge_config.challenger_reveal_timeout_slots(),
        expected_config.challenger_reveal_timeout_slots
    );
    assert_eq!(
        challenge_config.payout_timelock_slots(),
        expected_config.payout_timelock_slots
    );
    assert_eq!(
        challenge_config.verifiers_per_commitment(),
        expected_config.verifiers_per_commitment
    );
    assert_eq!(
        challenge_config.approval_threshold(),
        expected_config.approval_threshold
    );
    assert_eq!(
        challenge_config.max_window_extensions(),
        expected_config.max_window_extensions
    );
    assert_eq!(
        challenge_config.match_penalty_bps(),
        expected_config.match_penalty_bps
    );

    let verifier_registry_account =
        banks.get_account(verifier_registry).await.unwrap().unwrap();
    let registry =
        VerifierRegistry::decode(&verifier_registry_account.data).unwrap();
    let (_, expected_registry_bump) = Pubkey::find_program_address(
        &[VERIFIER_REGISTRY_SEED, delegation_cluster.as_ref()],
        &dlp_api::id(),
    );

    assert_eq!(registry.discriminator(), VerifierRegistry::DISCRIMINATOR);
    assert_eq!(registry.bump(), expected_registry_bump);
    assert_eq!(*registry.delegation_cluster(), delegation_cluster);
    assert_eq!(registry.next_selection_index(), 0);
    assert!(registry.entries().is_empty());
}

#[tokio::test]
async fn test_init_direct_delegation_cluster() {
    let (banks, payer, config_authority, cluster_identity, blockhash) =
        setup_program_test_env().await;
    let args = direct_cluster_args();
    let delegation_cluster = delegation_cluster_pda(&cluster_identity.pubkey());
    let verifier_registry = verifier_registry_pda(&delegation_cluster);
    let ix = init_delegation_cluster(
        config_authority.pubkey(),
        cluster_identity.pubkey(),
        args,
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &config_authority, &cluster_identity],
        blockhash,
    );

    banks.process_transaction(tx).await.unwrap();

    let delegation_cluster_account = banks
        .get_account(delegation_cluster)
        .await
        .unwrap()
        .unwrap();
    let cluster =
        DelegationCluster::decode(&delegation_cluster_account.data).unwrap();

    assert_eq!(
        delegation_cluster_account.data.len(),
        DelegationCluster::MIN_DATA_LEN
    );
    assert!(cluster.challenge_config().is_none());
    assert!(banks
        .get_account(verifier_registry)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn test_init_delegation_cluster_fails_twice() {
    let (banks, payer, config_authority, cluster_identity, blockhash) =
        setup_program_test_env().await;
    let args = challenge_cluster_args();

    let tx = Transaction::new_signed_with_payer(
        &[init_delegation_cluster(
            config_authority.pubkey(),
            cluster_identity.pubkey(),
            args.clone(),
        )],
        Some(&payer.pubkey()),
        &[&payer, &config_authority, &cluster_identity],
        blockhash,
    );
    banks.process_transaction(tx).await.unwrap();

    let tx = Transaction::new_signed_with_payer(
        &[init_delegation_cluster(
            config_authority.pubkey(),
            cluster_identity.pubkey(),
            args,
        )],
        Some(&payer.pubkey()),
        &[&payer, &config_authority, &cluster_identity],
        banks.get_latest_blockhash().await.unwrap(),
    );

    assert!(banks.process_transaction(tx).await.is_err());
}

#[tokio::test]
async fn test_init_delegation_cluster_rejects_wrong_cluster_pda() {
    let (banks, payer, config_authority, cluster_identity, blockhash) =
        setup_program_test_env().await;
    let mut ix = init_delegation_cluster(
        config_authority.pubkey(),
        cluster_identity.pubkey(),
        challenge_cluster_args(),
    );

    ix.accounts[2].pubkey = Pubkey::new_unique();

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &config_authority, &cluster_identity],
        blockhash,
    );

    assert!(banks.process_transaction(tx).await.is_err());
}

#[tokio::test]
async fn test_init_delegation_cluster_rejects_wrong_registry_pda() {
    let (banks, payer, config_authority, cluster_identity, blockhash) =
        setup_program_test_env().await;
    let mut ix = init_delegation_cluster(
        config_authority.pubkey(),
        cluster_identity.pubkey(),
        challenge_cluster_args(),
    );

    ix.accounts[4].pubkey = Pubkey::new_unique();

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &config_authority, &cluster_identity],
        blockhash,
    );

    assert!(banks.process_transaction(tx).await.is_err());
}

#[tokio::test]
async fn test_init_delegation_cluster_requires_authority_signature() {
    let (banks, payer, config_authority, cluster_identity, blockhash) =
        setup_program_test_env().await;
    let mut ix = init_delegation_cluster(
        config_authority.pubkey(),
        cluster_identity.pubkey(),
        challenge_cluster_args(),
    );

    ix.accounts[0].is_signer = false;

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &cluster_identity],
        blockhash,
    );

    assert!(banks.process_transaction(tx).await.is_err());
}

#[tokio::test]
async fn test_init_delegation_cluster_requires_cluster_identity_signature() {
    let (banks, payer, config_authority, cluster_identity, blockhash) =
        setup_program_test_env().await;
    let mut ix = init_delegation_cluster(
        config_authority.pubkey(),
        cluster_identity.pubkey(),
        challenge_cluster_args(),
    );

    ix.accounts[1].is_signer = false;

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &config_authority],
        blockhash,
    );

    assert!(banks.process_transaction(tx).await.is_err());
}

#[tokio::test]
async fn test_init_delegation_cluster_rejects_invalid_challenge_config() {
    let (banks, payer, config_authority, cluster_identity, blockhash) =
        setup_program_test_env().await;
    let mut args = challenge_cluster_args();

    args.challenge_config
        .as_mut()
        .unwrap()
        .verifiers_per_commitment = 2;

    let tx = Transaction::new_signed_with_payer(
        &[init_delegation_cluster(
            config_authority.pubkey(),
            cluster_identity.pubkey(),
            args,
        )],
        Some(&payer.pubkey()),
        &[&payer, &config_authority, &cluster_identity],
        blockhash,
    );

    assert!(banks.process_transaction(tx).await.is_err());
}

fn direct_cluster_args() -> InitDelegationClusterArgs {
    InitDelegationClusterArgs {
        current_validator: Pubkey::new_unique(),
        validator_rotation_authority: Pubkey::new_unique(),
        challenge_config: None,
    }
}

fn challenge_cluster_args() -> InitDelegationClusterArgs {
    InitDelegationClusterArgs {
        challenge_config: Some(ChallengeConfig {
            resolver: Pubkey::new_unique(),
            min_operator_bond: 1,
            min_verifier_bond: 1,
            min_challenger_stake: 1,
            challenge_window_slots: 10,
            operator_response_timeout_slots: 10,
            challenger_reveal_timeout_slots: 10,
            payout_timelock_slots: 10,
            verifiers_per_commitment: 1,
            approval_threshold: 1,
            max_window_extensions: 1,
            match_penalty_bps: 500,
        }),
        ..direct_cluster_args()
    }
}

async fn setup_program_test_env(
) -> (BanksClient, Keypair, Keypair, Keypair, Hash) {
    let mut program_test = ProgramTest::new("dlp", dlp_api::ID, None);
    program_test.prefer_bpf(true);

    let config_authority = Keypair::new();
    let cluster_identity = Keypair::new();
    program_test.add_account(
        config_authority.pubkey(),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
    program_test.add_account(
        cluster_identity.pubkey(),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks, payer, blockhash) = program_test.start().await;
    (banks, payer, config_authority, cluster_identity, blockhash)
}
