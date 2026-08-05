use dlp::solana_program;
use dlp_api::{
    pda::fees_vault_pda,
    v2::{
        instruction_builder::init_protocol_config,
        pda::{protocol_config_pda, verifier_registry_pda},
        InitProtocolConfigArgs, ProtocolConfig, VerifierRegistry,
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

mod fixtures;

#[tokio::test]
async fn test_v2_init_protocol_config() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;

    let args = valid_args();
    let ix = init_protocol_config(authority.pubkey(), args.clone());
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &authority],
        blockhash,
    );

    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok());

    let protocol_config_account = banks
        .get_account(protocol_config_pda())
        .await
        .unwrap()
        .unwrap();
    let protocol_config = ProtocolConfig::try_from_bytes_with_discriminator(
        &protocol_config_account.data,
    )
    .unwrap();

    assert_eq!(protocol_config.authority, authority.pubkey());
    assert!(!protocol_config.paused);
    assert_eq!(protocol_config.vrf_program, args.vrf_program);
    assert_eq!(protocol_config.vrf_config, args.vrf_config);
    assert_eq!(protocol_config.resolver, args.resolver);
    assert_eq!(protocol_config.protocol_fee_vault, fees_vault_pda());
    assert_eq!(protocol_config.min_operator_bond, args.min_operator_bond);
    assert_eq!(protocol_config.min_verifier_bond, args.min_verifier_bond);
    assert_eq!(
        protocol_config.min_challenger_stake,
        args.min_challenger_stake
    );
    assert_eq!(
        protocol_config.challenge_window_slots,
        args.challenge_window_slots
    );
    assert_eq!(
        protocol_config.operator_response_timeout_slots,
        args.operator_response_timeout_slots
    );
    assert_eq!(
        protocol_config.challenger_reveal_timeout_slots,
        args.challenger_reveal_timeout_slots
    );
    assert_eq!(
        protocol_config.payout_timelock_slots,
        args.payout_timelock_slots
    );
    assert_eq!(
        protocol_config.selected_verifier_count,
        args.selected_verifier_count
    );
    assert_eq!(protocol_config.approval_threshold, args.approval_threshold);
    assert_eq!(
        protocol_config.max_window_extensions,
        args.max_window_extensions
    );
    assert_eq!(protocol_config.match_penalty_bps, args.match_penalty_bps);

    let verifier_registry_account = banks
        .get_account(verifier_registry_pda())
        .await
        .unwrap()
        .unwrap();
    let verifier_registry =
        VerifierRegistry::try_from_bytes_with_discriminator(
            &verifier_registry_account.data,
        )
        .unwrap();

    assert_eq!(verifier_registry.registry_revision, 0);
    assert!(verifier_registry.entries.is_empty());
}

#[tokio::test]
async fn test_v2_init_protocol_config_fails_twice() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;

    let ix = init_protocol_config(authority.pubkey(), valid_args());
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &authority],
        blockhash,
    );
    banks.process_transaction(tx).await.unwrap();

    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let ix = init_protocol_config(authority.pubkey(), valid_args());
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &authority],
        blockhash,
    );

    assert!(banks.process_transaction(tx).await.is_err());
}

#[tokio::test]
async fn test_v2_init_protocol_config_fails_with_wrong_protocol_config_pda() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;

    let mut ix = init_protocol_config(authority.pubkey(), valid_args());
    ix.accounts[1].pubkey = Pubkey::new_unique();

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &authority],
        blockhash,
    );

    assert!(banks.process_transaction(tx).await.is_err());
}

#[tokio::test]
async fn test_v2_init_protocol_config_fails_with_wrong_verifier_registry_pda() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;

    let mut ix = init_protocol_config(authority.pubkey(), valid_args());
    ix.accounts[2].pubkey = Pubkey::new_unique();

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &authority],
        blockhash,
    );

    assert!(banks.process_transaction(tx).await.is_err());
}

#[tokio::test]
async fn test_v2_init_protocol_config_fails_without_authority_signature() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;

    let mut ix = init_protocol_config(authority.pubkey(), valid_args());
    ix.accounts[0].is_signer = false;

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer],
        blockhash,
    );

    assert!(banks.process_transaction(tx).await.is_err());
}

#[tokio::test]
async fn test_v2_init_protocol_config_fails_with_invalid_args() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;

    let mut args = valid_args();
    args.approval_threshold = args.selected_verifier_count + 1;

    let ix = init_protocol_config(authority.pubkey(), args);
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &authority],
        blockhash,
    );

    assert!(banks.process_transaction(tx).await.is_err());
}

#[tokio::test]
async fn test_v1_dispatch_still_works_after_v2_routing() {
    let (banks, payer, _authority, blockhash) = setup_program_test_env().await;

    let ix =
        dlp_api::instruction_builder::init_protocol_fees_vault(payer.pubkey());
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer],
        blockhash,
    );

    assert!(banks.process_transaction(tx).await.is_ok());
}

fn valid_args() -> InitProtocolConfigArgs {
    InitProtocolConfigArgs {
        vrf_program: Pubkey::new_unique(),
        vrf_config: Pubkey::new_unique(),
        resolver: Pubkey::new_unique(),
        min_operator_bond: 1,
        min_verifier_bond: 1,
        min_challenger_stake: 1,
        challenge_window_slots: 10,
        operator_response_timeout_slots: 10,
        challenger_reveal_timeout_slots: 10,
        payout_timelock_slots: 10,
        selected_verifier_count: 3,
        approval_threshold: 2,
        max_window_extensions: 1,
        match_penalty_bps: 500,
    }
}

async fn setup_program_test_env() -> (BanksClient, Keypair, Keypair, Hash) {
    let mut program_test = ProgramTest::new("dlp", dlp_api::ID, None);
    program_test.prefer_bpf(true);

    let authority = Keypair::new();
    program_test.add_account(
        authority.pubkey(),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks, payer, blockhash) = program_test.start().await;
    (banks, payer, authority, blockhash)
}
