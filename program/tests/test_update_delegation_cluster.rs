use delegation_program_interface as dpi;
use dpi::{
    pda::fees_vault_pda,
    v2::{
        instruction_builder::update_delegation_cluster,
        pda::{delegation_cluster_pda, DELEGATION_CLUSTER_SEED},
        ChallengeConfig, DelegationCluster, UpdateDelegationClusterArgs,
    },
};
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use wheels::layout::Decodable;

mod fixtures;

use crate::fixtures::v2::{
    challenge_cluster_args, direct_cluster_args, initialize_delegation_cluster,
    initialize_delegation_cluster_with_identity, setup_program_test_env,
};

#[tokio::test]
async fn test_update_delegation_cluster() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;
    let initial_args = challenge_cluster_args(Pubkey::new_unique());
    let initial_config = initial_args.challenge_config.as_ref().unwrap();
    let update_args = UpdateDelegationClusterArgs {
        challenge_config: Some(updated_challenge_config(initial_config)),
    };
    let cluster_identity = Keypair::new();
    let cluster_id = cluster_identity.pubkey();
    let current_validator = initial_args.current_validator;
    let validator_rotation_authority =
        initial_args.validator_rotation_authority;
    let delegation_cluster = delegation_cluster_pda(&cluster_id);

    initialize_delegation_cluster_with_identity(
        &banks,
        &payer,
        &authority,
        &cluster_identity,
        blockhash,
        initial_args,
    )
    .await;

    let ix = update_delegation_cluster(
        authority.pubkey(),
        delegation_cluster,
        update_args.clone(),
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &authority],
        banks.get_latest_blockhash().await.unwrap(),
    );

    banks.process_transaction(tx).await.unwrap();

    let account = banks
        .get_account(delegation_cluster)
        .await
        .unwrap()
        .unwrap();
    let cluster = DelegationCluster::decode(&account.data).unwrap();
    let challenge_config = cluster.challenge_config().unwrap();
    let expected_config = update_args.challenge_config.as_ref().unwrap();
    let (_, expected_bump) = Pubkey::find_program_address(
        &[DELEGATION_CLUSTER_SEED, cluster_id.as_ref()],
        &dpi::id(),
    );

    assert_eq!(cluster.discriminator(), DelegationCluster::DISCRIMINATOR);
    assert_eq!(cluster.bump(), expected_bump);
    assert_eq!(*cluster.cluster_id(), cluster_id);
    assert_eq!(*cluster.config_authority(), authority.pubkey());
    assert_eq!(
        *cluster.validator_rotation_authority(),
        validator_rotation_authority
    );
    assert_eq!(*cluster.current_validator(), current_validator);
    assert!(!cluster.paused());
    assert_eq!(*cluster.protocol_fee_vault(), fees_vault_pda());
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
}

#[tokio::test]
async fn test_update_delegation_cluster_fails_with_wrong_authority() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;
    let initial_args = challenge_cluster_args(Pubkey::new_unique());
    let update_args = UpdateDelegationClusterArgs {
        challenge_config: Some(updated_challenge_config(
            initial_args.challenge_config.as_ref().unwrap(),
        )),
    };
    let delegation_cluster = initialize_delegation_cluster(
        &banks,
        &payer,
        &authority,
        blockhash,
        initial_args,
    )
    .await;
    let wrong_authority = Keypair::new();
    let ix = update_delegation_cluster(
        wrong_authority.pubkey(),
        delegation_cluster,
        update_args,
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &wrong_authority],
        banks.get_latest_blockhash().await.unwrap(),
    );

    assert!(banks.process_transaction(tx).await.is_err());
}

#[tokio::test]
async fn test_update_delegation_cluster_fails_with_invalid_args() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;
    let initial_args = challenge_cluster_args(Pubkey::new_unique());
    let mut update_config = updated_challenge_config(
        initial_args.challenge_config.as_ref().unwrap(),
    );
    update_config.approval_threshold =
        update_config.verifiers_per_commitment + 1;
    let delegation_cluster = initialize_delegation_cluster(
        &banks,
        &payer,
        &authority,
        blockhash,
        initial_args,
    )
    .await;
    let ix = update_delegation_cluster(
        authority.pubkey(),
        delegation_cluster,
        UpdateDelegationClusterArgs {
            challenge_config: Some(update_config),
        },
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &authority],
        banks.get_latest_blockhash().await.unwrap(),
    );

    assert!(banks.process_transaction(tx).await.is_err());
}

#[tokio::test]
async fn test_update_delegation_cluster_fails_with_wrong_pda() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;
    let initial_args = challenge_cluster_args(Pubkey::new_unique());
    let update_args = UpdateDelegationClusterArgs {
        challenge_config: Some(updated_challenge_config(
            initial_args.challenge_config.as_ref().unwrap(),
        )),
    };
    let delegation_cluster = initialize_delegation_cluster(
        &banks,
        &payer,
        &authority,
        blockhash,
        initial_args,
    )
    .await;
    let mut ix = update_delegation_cluster(
        authority.pubkey(),
        delegation_cluster,
        update_args,
    );
    ix.accounts[1].pubkey = Pubkey::new_unique();
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &authority],
        banks.get_latest_blockhash().await.unwrap(),
    );

    assert!(banks.process_transaction(tx).await.is_err());
}

#[tokio::test]
async fn test_update_delegation_cluster_rejects_direct_cluster() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;
    let current_validator = Pubkey::new_unique();
    let delegation_cluster = initialize_delegation_cluster(
        &banks,
        &payer,
        &authority,
        blockhash,
        direct_cluster_args(current_validator),
    )
    .await;
    let challenge_config = challenge_cluster_args(current_validator)
        .challenge_config
        .unwrap();
    let ix = update_delegation_cluster(
        authority.pubkey(),
        delegation_cluster,
        UpdateDelegationClusterArgs {
            challenge_config: Some(challenge_config),
        },
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &authority],
        banks.get_latest_blockhash().await.unwrap(),
    );

    assert!(banks.process_transaction(tx).await.is_err());
}

fn updated_challenge_config(initial: &ChallengeConfig) -> ChallengeConfig {
    ChallengeConfig {
        resolver: Pubkey::new_unique(),
        min_operator_bond: initial.min_operator_bond + 10,
        min_verifier_bond: initial.min_verifier_bond + 12,
        min_challenger_stake: initial.min_challenger_stake + 16,
        challenge_window_slots: initial.challenge_window_slots + 9,
        operator_response_timeout_slots: initial
            .operator_response_timeout_slots
            + 13,
        challenger_reveal_timeout_slots: initial
            .challenger_reveal_timeout_slots
            + 19,
        payout_timelock_slots: initial.payout_timelock_slots + 21,
        verifiers_per_commitment: initial.verifiers_per_commitment,
        approval_threshold: initial.approval_threshold,
        max_window_extensions: initial.max_window_extensions + 1,
        match_penalty_bps: initial.match_penalty_bps + 200,
    }
}
