use dlp_api::{
    pda::fees_vault_pda,
    v2::{
        instruction_builder::update_protocol_config, pda::protocol_config_pda,
        ProtocolConfig,
    },
};
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use wheels::layout::Decodable;

mod fixtures;

use crate::fixtures::v2::{init_v2, setup_program_test_env, valid_args};

#[tokio::test]
async fn test_v2_update_protocol_config() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;
    let initial_args = valid_args();

    init_v2(&banks, &payer, &authority, blockhash, initial_args).await;

    let mut update_args = valid_args();
    update_args.resolver = Pubkey::new_unique();
    update_args.min_operator_bond = 11;
    update_args.min_verifier_bond = 13;
    update_args.min_challenger_stake = 17;
    update_args.challenge_window_slots = 19;
    update_args.operator_response_timeout_slots = 23;
    update_args.challenger_reveal_timeout_slots = 29;
    update_args.payout_timelock_slots = 31;
    update_args.verifiers_per_commitment = 5;
    update_args.approval_threshold = 3;
    update_args.max_window_extensions = 2;
    update_args.match_penalty_bps = 700;

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
        <ProtocolConfig as Decodable>::decode(&protocol_config_account.data)
            .unwrap();

    assert_eq!(
        protocol_config.discriminator(),
        ProtocolConfig::DISCRIMINATOR
    );
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
async fn test_v2_update_protocol_config_fails_with_wrong_authority() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;

    init_v2(&banks, &payer, &authority, blockhash, valid_args()).await;

    let wrong_authority = Keypair::new();
    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let ix = update_protocol_config(wrong_authority.pubkey(), valid_args());
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &wrong_authority],
        blockhash,
    );

    assert!(banks.process_transaction(tx).await.is_err());
}

#[tokio::test]
async fn test_v2_update_protocol_config_fails_with_invalid_args() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;

    init_v2(&banks, &payer, &authority, blockhash, valid_args()).await;

    let mut args = valid_args();
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
async fn test_v2_update_protocol_config_fails_with_wrong_protocol_config_pda() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;

    init_v2(&banks, &payer, &authority, blockhash, valid_args()).await;

    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let mut ix = update_protocol_config(authority.pubkey(), valid_args());
    ix.accounts[1].pubkey = Pubkey::new_unique();
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &authority],
        blockhash,
    );

    assert!(banks.process_transaction(tx).await.is_err());
}
