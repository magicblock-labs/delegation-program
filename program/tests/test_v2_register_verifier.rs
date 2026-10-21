use delegation_program_interface::v2::{
    instruction_builder::register_verifier, pda::VERIFIER_BOND_SEED,
    RegisterVerifierArgs, VerifierBond, VerifierStatus,
};
use solana_program::{
    native_token::LAMPORTS_PER_SOL, pubkey::Pubkey, rent::Rent,
};
use solana_program_test::ProgramTestBanksClientExt;
use solana_sdk::{
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use solana_system_interface::instruction as system_instruction;
use wheels::layout::Decodable;

mod fixtures;

use crate::fixtures::v2::{
    challenge_cluster_args, direct_cluster_args, initialize_delegation_cluster,
    setup_program_test_env,
};

#[tokio::test]
async fn test_register_verifier() {
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
    let (verifier_bond_address, expected_verifier_bond_bump) =
        Pubkey::find_program_address(
            &[
                VERIFIER_BOND_SEED,
                delegation_cluster.as_ref(),
                verifier.pubkey().as_ref(),
            ],
            &delegation_program_interface::id(),
        );

    fund_verifier(&banks, &payer, &verifier).await;

    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let ix = register_verifier(
        verifier.pubkey(),
        authority.pubkey(),
        delegation_cluster,
        RegisterVerifierArgs {
            stake_lamports: min_verifier_bond,
        },
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &verifier, &authority],
        blockhash,
    );

    assert!(banks.process_transaction(tx).await.is_ok());

    let verifier_bond_account = banks
        .get_account(verifier_bond_address)
        .await
        .unwrap()
        .unwrap();
    let verifier_bond =
        VerifierBond::decode(&verifier_bond_account.data).unwrap();
    let expected_verifier_bond_lamports = Rent::default()
        .minimum_balance(VerifierBond::DATA_LEN)
        + min_verifier_bond;

    assert_eq!(verifier_bond.discriminator(), VerifierBond::DISCRIMINATOR);
    assert_eq!(verifier_bond.bump(), expected_verifier_bond_bump);
    assert_eq!(*verifier_bond.delegation_cluster(), delegation_cluster);
    assert_eq!(*verifier_bond.verifier_identity(), verifier.pubkey());
    assert_eq!(verifier_bond.stake_lamports(), min_verifier_bond);
    assert_eq!(verifier_bond.status(), VerifierStatus::Active.value());
    assert!(verifier_bond.registered_slot() > 0);
    assert_eq!(verifier_bond.withdraw_requested_slot(), None);
    assert_eq!(
        verifier_bond_account.lamports,
        expected_verifier_bond_lamports
    );
}

#[tokio::test]
async fn test_register_verifier_fails_with_wrong_authority() {
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
    fund_verifier(&banks, &payer, &verifier).await;

    let wrong_authority = Keypair::new();
    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let ix = register_verifier(
        verifier.pubkey(),
        wrong_authority.pubkey(),
        delegation_cluster,
        RegisterVerifierArgs {
            stake_lamports: min_verifier_bond,
        },
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &verifier, &wrong_authority],
        blockhash,
    );

    assert!(banks.process_transaction(tx).await.is_err());
}

#[tokio::test]
async fn test_register_verifier_fails_with_low_stake() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;
    let verifier = Keypair::new();
    let cluster_args = challenge_cluster_args(Pubkey::new_unique());

    let delegation_cluster = initialize_delegation_cluster(
        &banks,
        &payer,
        &authority,
        blockhash,
        cluster_args,
    )
    .await;
    fund_verifier(&banks, &payer, &verifier).await;

    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let ix = register_verifier(
        verifier.pubkey(),
        authority.pubkey(),
        delegation_cluster,
        RegisterVerifierArgs { stake_lamports: 0 },
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &verifier, &authority],
        blockhash,
    );

    assert!(banks.process_transaction(tx).await.is_err());
}

#[tokio::test]
async fn test_register_verifier_fails_twice() {
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
    fund_verifier(&banks, &payer, &verifier).await;

    {
        let ix = register_verifier(
            verifier.pubkey(),
            authority.pubkey(),
            delegation_cluster,
            RegisterVerifierArgs {
                stake_lamports: min_verifier_bond,
            },
        );
        let latest_blockhash = banks.get_latest_blockhash().await.unwrap();
        let blockhash = banks
            .get_new_latest_blockhash(&latest_blockhash)
            .await
            .unwrap();
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer, &verifier, &authority],
            blockhash,
        );
        banks.process_transaction(tx).await.unwrap();
    }

    {
        let ix = register_verifier(
            verifier.pubkey(),
            authority.pubkey(),
            delegation_cluster,
            RegisterVerifierArgs {
                stake_lamports: min_verifier_bond,
            },
        );
        let latest_blockhash = banks.get_latest_blockhash().await.unwrap();
        let blockhash = banks
            .get_new_latest_blockhash(&latest_blockhash)
            .await
            .unwrap();
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer, &verifier, &authority],
            blockhash,
        );

        assert!(banks.process_transaction(tx).await.is_err());
    }
}

#[tokio::test]
async fn test_register_verifier_rejects_direct_cluster() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;
    let verifier = Keypair::new();
    let delegation_cluster = initialize_delegation_cluster(
        &banks,
        &payer,
        &authority,
        blockhash,
        direct_cluster_args(Pubkey::new_unique()),
    )
    .await;
    fund_verifier(&banks, &payer, &verifier).await;

    let ix = register_verifier(
        verifier.pubkey(),
        authority.pubkey(),
        delegation_cluster,
        RegisterVerifierArgs { stake_lamports: 1 },
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &verifier, &authority],
        banks.get_latest_blockhash().await.unwrap(),
    );

    assert!(banks.process_transaction(tx).await.is_err());
}

async fn fund_verifier(
    banks: &solana_program_test::BanksClient,
    payer: &Keypair,
    verifier: &Keypair,
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
}
