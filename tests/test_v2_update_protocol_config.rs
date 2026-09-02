use dlp_api::{
    pda::fees_vault_pda,
    v2::{
        instruction_builder::update_protocol_config,
        pda::{protocol_config_pda, PROTOCOL_CONFIG_SEED},
        ProtocolConfig, UpdateProtocolConfigArgs,
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
    initialize_protocol_config, setup_program_test_env,
    valid_protocol_config_args,
};

#[tokio::test]
async fn test_update_protocol_config() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;
    let initial_args = valid_protocol_config_args();

    initialize_protocol_config(
        &banks,
        &payer,
        &authority,
        blockhash,
        initial_args.clone(),
    )
    .await;

    let update_args = UpdateProtocolConfigArgs {
        resolver: Pubkey::new_unique(),
        min_operator_bond: initial_args.min_operator_bond + 10,
        min_verifier_bond: initial_args.min_verifier_bond + 12,
        min_challenger_stake: initial_args.min_challenger_stake + 16,
        challenge_window_slots: initial_args.challenge_window_slots + 9,
        operator_response_timeout_slots: initial_args
            .operator_response_timeout_slots
            + 13,
        challenger_reveal_timeout_slots: initial_args
            .challenger_reveal_timeout_slots
            + 19,
        payout_timelock_slots: initial_args.payout_timelock_slots + 21,
        verifiers_per_commitment: initial_args.verifiers_per_commitment,
        approval_threshold: initial_args.approval_threshold,
        max_window_extensions: initial_args.max_window_extensions + 1,
        match_penalty_bps: initial_args.match_penalty_bps + 200,
    };

    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let ix = update_protocol_config(authority.pubkey(), update_args.clone());
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &authority],
        blockhash,
    );

    assert!(banks.process_transaction(tx).await.is_ok());

    let protocol_config_account = banks
        .get_account(protocol_config_pda())
        .await
        .unwrap()
        .unwrap();
    let protocol_config =
        ProtocolConfig::decode(&protocol_config_account.data).unwrap();
    let (_, expected_protocol_config_bump) =
        Pubkey::find_program_address(&[PROTOCOL_CONFIG_SEED], &dlp_api::id());

    assert_eq!(
        protocol_config.discriminator(),
        ProtocolConfig::DISCRIMINATOR
    );
    assert_eq!(protocol_config.bump(), expected_protocol_config_bump);
    assert_eq!(*protocol_config.authority(), authority.pubkey());
    assert!(!protocol_config.paused());
    assert_eq!(*protocol_config.resolver(), update_args.resolver);
    assert_eq!(*protocol_config.protocol_fee_vault(), fees_vault_pda());
    assert_eq!(
        protocol_config.min_operator_bond(),
        update_args.min_operator_bond
    );
    assert_eq!(
        protocol_config.min_verifier_bond(),
        update_args.min_verifier_bond
    );
    assert_eq!(
        protocol_config.min_challenger_stake(),
        update_args.min_challenger_stake
    );
    assert_eq!(
        protocol_config.challenge_window_slots(),
        update_args.challenge_window_slots
    );
    assert_eq!(
        protocol_config.operator_response_timeout_slots(),
        update_args.operator_response_timeout_slots
    );
    assert_eq!(
        protocol_config.challenger_reveal_timeout_slots(),
        update_args.challenger_reveal_timeout_slots
    );
    assert_eq!(
        protocol_config.payout_timelock_slots(),
        update_args.payout_timelock_slots
    );
    assert_eq!(
        protocol_config.verifiers_per_commitment(),
        update_args.verifiers_per_commitment
    );
    assert_eq!(
        protocol_config.approval_threshold(),
        update_args.approval_threshold
    );
    assert_eq!(
        protocol_config.max_window_extensions(),
        update_args.max_window_extensions
    );
    assert_eq!(
        protocol_config.match_penalty_bps(),
        update_args.match_penalty_bps
    );
}

#[tokio::test]
async fn test_update_protocol_config_fails_with_wrong_authority() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;

    initialize_protocol_config(
        &banks,
        &payer,
        &authority,
        blockhash,
        valid_protocol_config_args(),
    )
    .await;

    let wrong_authority = Keypair::new();
    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let ix = update_protocol_config(
        wrong_authority.pubkey(),
        valid_protocol_config_args(),
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &wrong_authority],
        blockhash,
    );

    assert!(banks.process_transaction(tx).await.is_err());
}

#[tokio::test]
async fn test_update_protocol_config_fails_with_invalid_args() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;

    initialize_protocol_config(
        &banks,
        &payer,
        &authority,
        blockhash,
        valid_protocol_config_args(),
    )
    .await;

    let mut args = valid_protocol_config_args();
    args.approval_threshold = args.verifiers_per_commitment + 1;

    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let ix = update_protocol_config(authority.pubkey(), args);
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &authority],
        blockhash,
    );

    assert!(banks.process_transaction(tx).await.is_err());
}

#[tokio::test]
async fn test_update_protocol_config_fails_with_wrong_protocol_config_pda() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;

    initialize_protocol_config(
        &banks,
        &payer,
        &authority,
        blockhash,
        valid_protocol_config_args(),
    )
    .await;

    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let mut ix = update_protocol_config(
        authority.pubkey(),
        valid_protocol_config_args(),
    );
    ix.accounts[1].pubkey = Pubkey::new_unique();
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &authority],
        blockhash,
    );

    assert!(banks.process_transaction(tx).await.is_err());
}
