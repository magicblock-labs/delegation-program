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
    challenge_cluster_args, initialize_delegation_cluster,
    setup_program_test_env,
};

#[tokio::test]
async fn test_update_verifier_registry_adds_verifier() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;
    let verifier = Keypair::new();
    let cluster_args = challenge_cluster_args(Pubkey::new_unique());
    let min_verifier_bond = cluster_args
        .challenge_config
        .as_ref()
        .unwrap()
        .min_verifier_bond;
    let delegation_cluster =
        dlp_api::v2::pda::delegation_cluster_pda(&cluster_args.cluster_id);
    let (_, expected_verifier_registry_bump) = Pubkey::find_program_address(
        &[VERIFIER_REGISTRY_SEED, delegation_cluster.as_ref()],
        &dlp_api::id(),
    );

    initialize_delegation_cluster(
        &banks,
        &payer,
        &authority,
        blockhash,
        cluster_args,
    )
    .await;
    fund_and_register_verifier(
        &banks,
        &payer,
        &verifier,
        &authority,
        delegation_cluster,
        min_verifier_bond,
    )
    .await;

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
        &[&payer, &authority],
        blockhash,
    );

    assert!(banks.process_transaction(tx).await.is_ok());

    let verifier_registry_account = banks
        .get_account(verifier_registry_pda(&delegation_cluster))
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
    assert_eq!(*verifier_registry.delegation_cluster(), delegation_cluster);
    assert_eq!(verifier_registry.entries().len(), 1);
    let entry = verifier_registry.entries().iter().next().unwrap();
    assert_eq!(*entry.verifier_identity(), verifier.pubkey());
    assert_eq!(
        *entry.verifier_bond(),
        verifier_bond_pda(&delegation_cluster, &verifier.pubkey())
    );
    assert_eq!(entry.weight(), 1);
}

#[tokio::test]
async fn test_update_verifier_registry_fails_twice() {
    let (mut banks, payer, authority, blockhash) =
        setup_program_test_env().await;
    let verifier = Keypair::new();
    let cluster_args = challenge_cluster_args(Pubkey::new_unique());
    let min_verifier_bond = cluster_args
        .challenge_config
        .as_ref()
        .unwrap()
        .min_verifier_bond;

    let delegation_cluster = initialize_delegation_cluster(
        &banks,
        &payer,
        &authority,
        blockhash,
        cluster_args,
    )
    .await;
    fund_and_register_verifier(
        &banks,
        &payer,
        &verifier,
        &authority,
        delegation_cluster,
        min_verifier_bond,
    )
    .await;
    add_verifier_to_registry(
        &banks,
        &payer,
        &verifier,
        &authority,
        delegation_cluster,
        1,
    )
    .await;

    let latest_blockhash = banks.get_latest_blockhash().await.unwrap();
    let blockhash = banks
        .get_new_latest_blockhash(&latest_blockhash)
        .await
        .unwrap();
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
        &[&payer, &authority],
        blockhash,
    );

    assert!(banks.process_transaction(tx).await.is_err());
}

#[tokio::test]
async fn test_update_verifier_registry_fails_with_wrong_authority() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;
    let verifier = Keypair::new();
    let cluster_args = challenge_cluster_args(Pubkey::new_unique());
    let min_verifier_bond = cluster_args
        .challenge_config
        .as_ref()
        .unwrap()
        .min_verifier_bond;

    let delegation_cluster = initialize_delegation_cluster(
        &banks,
        &payer,
        &authority,
        blockhash,
        cluster_args,
    )
    .await;
    fund_and_register_verifier(
        &banks,
        &payer,
        &verifier,
        &authority,
        delegation_cluster,
        min_verifier_bond,
    )
    .await;

    let wrong_authority = Keypair::new();
    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let ix = update_verifier_registry(
        wrong_authority.pubkey(),
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
        &[&payer, &wrong_authority],
        blockhash,
    );

    assert!(banks.process_transaction(tx).await.is_err());
}

#[tokio::test]
async fn test_update_verifier_registry_rejects_cross_cluster_bond() {
    let (mut banks, payer, authority, blockhash) =
        setup_program_test_env().await;
    let verifier = Keypair::new();
    let first_cluster_args = challenge_cluster_args(Pubkey::new_unique());
    let second_cluster_args = challenge_cluster_args(Pubkey::new_unique());
    let min_verifier_bond = second_cluster_args
        .challenge_config
        .as_ref()
        .unwrap()
        .min_verifier_bond;
    let first_cluster = initialize_delegation_cluster(
        &banks,
        &payer,
        &authority,
        blockhash,
        first_cluster_args,
    )
    .await;
    let blockhash = banks.get_new_latest_blockhash(&blockhash).await.unwrap();
    let second_cluster = initialize_delegation_cluster(
        &banks,
        &payer,
        &authority,
        blockhash,
        second_cluster_args,
    )
    .await;
    fund_and_register_verifier(
        &banks,
        &payer,
        &verifier,
        &authority,
        second_cluster,
        min_verifier_bond,
    )
    .await;

    let mut ix = update_verifier_registry(
        authority.pubkey(),
        first_cluster,
        verifier.pubkey(),
        UpdateVerifierRegistryArgs {
            action: VerifierRegistryAction::Add.value(),
            weight: 1,
        },
    );
    ix.accounts[3].pubkey =
        verifier_bond_pda(&second_cluster, &verifier.pubkey());
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &authority],
        banks.get_latest_blockhash().await.unwrap(),
    );

    assert!(banks.process_transaction(tx).await.is_err());
}

#[tokio::test]
async fn test_update_verifier_registry_fails_with_invalid_weight() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;
    let verifier = Keypair::new();
    let cluster_args = challenge_cluster_args(Pubkey::new_unique());
    let min_verifier_bond = cluster_args
        .challenge_config
        .as_ref()
        .unwrap()
        .min_verifier_bond;

    let delegation_cluster = initialize_delegation_cluster(
        &banks,
        &payer,
        &authority,
        blockhash,
        cluster_args,
    )
    .await;
    fund_and_register_verifier(
        &banks,
        &payer,
        &verifier,
        &authority,
        delegation_cluster,
        min_verifier_bond,
    )
    .await;

    {
        let blockhash = banks.get_latest_blockhash().await.unwrap();
        let ix = update_verifier_registry(
            authority.pubkey(),
            delegation_cluster,
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
            delegation_cluster,
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
    let verifier = Keypair::new();
    let cluster_args = challenge_cluster_args(Pubkey::new_unique());
    let min_verifier_bond = cluster_args
        .challenge_config
        .as_ref()
        .unwrap()
        .min_verifier_bond;

    let delegation_cluster = initialize_delegation_cluster(
        &banks,
        &payer,
        &authority,
        blockhash,
        cluster_args,
    )
    .await;
    fund_and_register_verifier(
        &banks,
        &payer,
        &verifier,
        &authority,
        delegation_cluster,
        min_verifier_bond,
    )
    .await;

    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let ix = update_verifier_registry(
        authority.pubkey(),
        delegation_cluster,
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

async fn fund_and_register_verifier(
    banks: &solana_program_test::BanksClient,
    payer: &Keypair,
    verifier: &Keypair,
    authority: &Keypair,
    delegation_cluster: Pubkey,
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
}

async fn add_verifier_to_registry(
    banks: &solana_program_test::BanksClient,
    payer: &Keypair,
    verifier: &Keypair,
    authority: &Keypair,
    delegation_cluster: Pubkey,
    weight: u64,
) {
    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let ix = update_verifier_registry(
        authority.pubkey(),
        delegation_cluster,
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
