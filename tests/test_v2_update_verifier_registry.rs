use dlp_api::v2::{
    instruction_builder::{register_verifier, update_verifier_registry},
    pda::{verifier_bond_pda, verifier_registry_pda, VERIFIER_REGISTRY_SEED},
    RegisterVerifierArgs, UpdateVerifierRegistryArgs, VerifierRegistry,
    VerifierRegistryAction,
};
use solana_program::{native_token::LAMPORTS_PER_SOL, pubkey::Pubkey};
use solana_program_test::ProgramTestBanksClientExt;
use solana_sdk::{
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use solana_system_interface::instruction as system_instruction;
use wheels::layout::Decodable;

mod fixtures;

use crate::fixtures::v2::{
    initialize_protocol_config, setup_program_test_env,
    valid_protocol_config_args,
};

#[tokio::test]
async fn test_update_verifier_registry_adds_verifier() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;
    let config_args = valid_protocol_config_args();
    let verifier = Keypair::new();
    let (_, expected_verifier_registry_bump) =
        Pubkey::find_program_address(&[VERIFIER_REGISTRY_SEED], &dlp_api::id());

    initialize_protocol_config(
        &banks,
        &payer,
        &authority,
        blockhash,
        config_args.clone(),
    )
    .await;
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
            action: VerifierRegistryAction::Add.value(),
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
        VerifierRegistry::decode(&verifier_registry_account.data).unwrap();

    assert_eq!(
        verifier_registry.discriminator(),
        VerifierRegistry::DISCRIMINATOR
    );
    assert_eq!(verifier_registry.bump(), expected_verifier_registry_bump);
    assert_eq!(verifier_registry.entries().len(), 1);
    let entry = verifier_registry.entries().iter().next().unwrap();
    assert_eq!(*entry.verifier_identity(), verifier.pubkey());
    assert_eq!(
        *entry.verifier_bond(),
        verifier_bond_pda(&verifier.pubkey())
    );
    assert_eq!(entry.weight(), 1);
}

#[tokio::test]
async fn test_update_verifier_registry_fails_twice() {
    let (mut banks, payer, authority, blockhash) =
        setup_program_test_env().await;
    let config_args = valid_protocol_config_args();
    let verifier = Keypair::new();

    initialize_protocol_config(
        &banks,
        &payer,
        &authority,
        blockhash,
        config_args.clone(),
    )
    .await;
    register_v2_verifier(
        &banks,
        &payer,
        &verifier,
        &authority,
        config_args.min_verifier_bond,
    )
    .await;
    add_verifier_to_registry(&banks, &payer, &verifier, &authority, 1).await;

    let latest_blockhash = banks.get_latest_blockhash().await.unwrap();
    let blockhash = banks
        .get_new_latest_blockhash(&latest_blockhash)
        .await
        .unwrap();
    let ix = update_verifier_registry(
        authority.pubkey(),
        verifier.pubkey(),
        UpdateVerifierRegistryArgs {
            action: VerifierRegistryAction::Add.value(),
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
async fn test_update_verifier_registry_fails_with_wrong_authority() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;
    let config_args = valid_protocol_config_args();
    let verifier = Keypair::new();

    initialize_protocol_config(
        &banks,
        &payer,
        &authority,
        blockhash,
        config_args.clone(),
    )
    .await;
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
            action: VerifierRegistryAction::Add.value(),
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
async fn test_update_verifier_registry_fails_with_invalid_weight() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;
    let config_args = valid_protocol_config_args();
    let verifier = Keypair::new();

    initialize_protocol_config(
        &banks,
        &payer,
        &authority,
        blockhash,
        config_args.clone(),
    )
    .await;
    register_v2_verifier(
        &banks,
        &payer,
        &verifier,
        &authority,
        config_args.min_verifier_bond,
    )
    .await;

    {
        let blockhash = banks.get_latest_blockhash().await.unwrap();
        let ix = update_verifier_registry(
            authority.pubkey(),
            verifier.pubkey(),
            UpdateVerifierRegistryArgs {
                action: VerifierRegistryAction::Add.value(),
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

    {
        let blockhash = banks.get_latest_blockhash().await.unwrap();
        let ix = update_verifier_registry(
            authority.pubkey(),
            verifier.pubkey(),
            UpdateVerifierRegistryArgs {
                action: VerifierRegistryAction::Add.value(),
                weight: 2,
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
}

#[tokio::test]
async fn test_update_verifier_registry_fails_with_remove_action() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;
    let config_args = valid_protocol_config_args();
    let verifier = Keypair::new();

    initialize_protocol_config(
        &banks,
        &payer,
        &authority,
        blockhash,
        config_args.clone(),
    )
    .await;
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
            action: VerifierRegistryAction::Remove.value(),
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
    stake_lamports: u64,
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
        RegisterVerifierArgs { stake_lamports },
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
            action: VerifierRegistryAction::Add.value(),
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
