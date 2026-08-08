use dlp_api::v2::{
    instruction_builder::{register_verifier, update_verifier_registry},
    pda::{verifier_bond_pda, verifier_registry_pda},
    RegisterVerifierArgs, UpdateVerifierRegistryArgs, VerifierRegistry,
    VERIFIER_REGISTRY_ACTION_ADD, VERIFIER_REGISTRY_ACTION_REMOVE,
};
use solana_program::native_token::LAMPORTS_PER_SOL;
use solana_sdk::{
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use solana_system_interface::instruction as system_instruction;

mod fixtures;

use crate::fixtures::v2::{init_v2, setup_program_test_env, valid_args};

#[tokio::test]
async fn test_v2_update_verifier_registry_adds_verifier() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;
    let config_args = valid_args();
    let verifier = Keypair::new();

    init_v2(&banks, &payer, &authority, blockhash, config_args.clone()).await;
    register_v2_verifier(
        &banks,
        &payer,
        &verifier,
        &authority,
        config_args.min_verifier_bond,
    )
    .await;

    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let ix = update_verifier_registry(
        authority.pubkey(),
        verifier.pubkey(),
        UpdateVerifierRegistryArgs {
            action: VERIFIER_REGISTRY_ACTION_ADD,
            weight: 1,
        },
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &authority],
        blockhash,
    );

    assert!(banks.process_transaction(tx).await.is_ok());

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

    assert_eq!(verifier_registry.registry_revision, 1);
    assert_eq!(verifier_registry.entries.len(), 1);
    assert_eq!(
        verifier_registry.entries[0].verifier_identity,
        verifier.pubkey()
    );
    assert_eq!(
        verifier_registry.entries[0].verifier_bond,
        verifier_bond_pda(&verifier.pubkey())
    );
    assert_eq!(verifier_registry.entries[0].weight, 1);
}

#[tokio::test]
async fn test_v2_update_verifier_registry_fails_twice() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;
    let config_args = valid_args();
    let verifier = Keypair::new();

    init_v2(&banks, &payer, &authority, blockhash, config_args.clone()).await;
    register_v2_verifier(
        &banks,
        &payer,
        &verifier,
        &authority,
        config_args.min_verifier_bond,
    )
    .await;
    add_verifier_to_registry(&banks, &payer, &verifier, &authority, 1).await;

    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let ix = update_verifier_registry(
        authority.pubkey(),
        verifier.pubkey(),
        UpdateVerifierRegistryArgs {
            action: VERIFIER_REGISTRY_ACTION_ADD,
            weight: 1,
        },
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &authority],
        blockhash,
    );

    assert!(banks.process_transaction(tx).await.is_err());
}

#[tokio::test]
async fn test_v2_update_verifier_registry_fails_with_wrong_authority() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;
    let config_args = valid_args();
    let verifier = Keypair::new();

    init_v2(&banks, &payer, &authority, blockhash, config_args.clone()).await;
    register_v2_verifier(
        &banks,
        &payer,
        &verifier,
        &authority,
        config_args.min_verifier_bond,
    )
    .await;

    let wrong_authority = Keypair::new();
    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let ix = update_verifier_registry(
        wrong_authority.pubkey(),
        verifier.pubkey(),
        UpdateVerifierRegistryArgs {
            action: VERIFIER_REGISTRY_ACTION_ADD,
            weight: 1,
        },
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
async fn test_v2_update_verifier_registry_fails_with_zero_weight() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;
    let config_args = valid_args();
    let verifier = Keypair::new();

    init_v2(&banks, &payer, &authority, blockhash, config_args.clone()).await;
    register_v2_verifier(
        &banks,
        &payer,
        &verifier,
        &authority,
        config_args.min_verifier_bond,
    )
    .await;

    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let ix = update_verifier_registry(
        authority.pubkey(),
        verifier.pubkey(),
        UpdateVerifierRegistryArgs {
            action: VERIFIER_REGISTRY_ACTION_ADD,
            weight: 0,
        },
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &authority],
        blockhash,
    );

    assert!(banks.process_transaction(tx).await.is_err());
}

#[tokio::test]
async fn test_v2_update_verifier_registry_fails_with_remove_action() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;
    let config_args = valid_args();
    let verifier = Keypair::new();

    init_v2(&banks, &payer, &authority, blockhash, config_args.clone()).await;
    register_v2_verifier(
        &banks,
        &payer,
        &verifier,
        &authority,
        config_args.min_verifier_bond,
    )
    .await;

    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let ix = update_verifier_registry(
        authority.pubkey(),
        verifier.pubkey(),
        UpdateVerifierRegistryArgs {
            action: VERIFIER_REGISTRY_ACTION_REMOVE,
            weight: 1,
        },
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &authority],
        blockhash,
    );

    assert!(banks.process_transaction(tx).await.is_err());
}

async fn register_v2_verifier(
    banks: &solana_program_test::BanksClient,
    payer: &Keypair,
    verifier: &Keypair,
    authority: &Keypair,
    amount_lamports: u64,
) {
    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let ix = system_instruction::transfer(
        &payer.pubkey(),
        &verifier.pubkey(),
        LAMPORTS_PER_SOL,
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer],
        blockhash,
    );
    banks.process_transaction(tx).await.unwrap();

    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let ix = register_verifier(
        verifier.pubkey(),
        authority.pubkey(),
        RegisterVerifierArgs { amount_lamports },
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer, verifier, authority],
        blockhash,
    );

    banks.process_transaction(tx).await.unwrap();
}

async fn add_verifier_to_registry(
    banks: &solana_program_test::BanksClient,
    payer: &Keypair,
    verifier: &Keypair,
    authority: &Keypair,
    weight: u64,
) {
    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let ix = update_verifier_registry(
        authority.pubkey(),
        verifier.pubkey(),
        UpdateVerifierRegistryArgs {
            action: VERIFIER_REGISTRY_ACTION_ADD,
            weight,
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
