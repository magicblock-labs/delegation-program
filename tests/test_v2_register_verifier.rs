use dlp_api::v2::{
    instruction_builder::register_verifier, pda::verifier_bond_pda,
    RegisterVerifierArgs, VerifierBond, VERIFIER_STATUS_ACTIVE,
};
use solana_program::native_token::LAMPORTS_PER_SOL;
use solana_sdk::{
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use solana_system_interface::instruction as system_instruction;
use wheels::layout::Decodable;

mod fixtures;

use crate::fixtures::v2::{init_v2, setup_program_test_env, valid_args};

#[tokio::test]
async fn test_v2_register_verifier() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;
    let config_args = valid_args();
    let verifier = Keypair::new();

    init_v2(&banks, &payer, &authority, blockhash, config_args.clone()).await;
    fund_verifier(&banks, &payer, &verifier).await;

    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let ix = register_verifier(
        verifier.pubkey(),
        authority.pubkey(),
        RegisterVerifierArgs {
            amount_lamports: config_args.min_verifier_bond,
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
        .get_account(verifier_bond_pda(&verifier.pubkey()))
        .await
        .unwrap()
        .unwrap();
    let verifier_bond =
        <VerifierBond as Decodable>::decode(&verifier_bond_account.data)
            .unwrap();

    assert_eq!(verifier_bond.discriminator(), VerifierBond::DISCRIMINATOR);
    assert_eq!(*verifier_bond.verifier_identity(), verifier.pubkey());
    assert_eq!(
        verifier_bond.stake_lamports(),
        config_args.min_verifier_bond
    );
    assert_eq!(verifier_bond.status(), VERIFIER_STATUS_ACTIVE);
    assert!(verifier_bond.registered_slot() > 0);
    assert_eq!(verifier_bond.withdraw_requested_slot(), None);
    assert!(
        verifier_bond_account.lamports > verifier_bond.stake_lamports(),
        "verifier bond account should hold rent plus stake"
    );
}

#[tokio::test]
async fn test_v2_register_verifier_fails_with_wrong_authority() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;
    let config_args = valid_args();
    let verifier = Keypair::new();

    init_v2(&banks, &payer, &authority, blockhash, config_args.clone()).await;
    fund_verifier(&banks, &payer, &verifier).await;

    let wrong_authority = Keypair::new();
    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let ix = register_verifier(
        verifier.pubkey(),
        wrong_authority.pubkey(),
        RegisterVerifierArgs {
            amount_lamports: config_args.min_verifier_bond,
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
async fn test_v2_register_verifier_fails_with_low_stake() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;
    let config_args = valid_args();
    let verifier = Keypair::new();

    init_v2(&banks, &payer, &authority, blockhash, config_args).await;
    fund_verifier(&banks, &payer, &verifier).await;

    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let ix = register_verifier(
        verifier.pubkey(),
        authority.pubkey(),
        RegisterVerifierArgs { amount_lamports: 0 },
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
async fn test_v2_register_verifier_fails_twice() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;
    let config_args = valid_args();
    let verifier = Keypair::new();

    init_v2(&banks, &payer, &authority, blockhash, config_args.clone()).await;
    fund_verifier(&banks, &payer, &verifier).await;

    let ix = register_verifier(
        verifier.pubkey(),
        authority.pubkey(),
        RegisterVerifierArgs {
            amount_lamports: config_args.min_verifier_bond,
        },
    );
    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &verifier, &authority],
        blockhash,
    );
    banks.process_transaction(tx).await.unwrap();

    let ix = register_verifier(
        verifier.pubkey(),
        authority.pubkey(),
        RegisterVerifierArgs {
            amount_lamports: config_args.min_verifier_bond,
        },
    );
    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &verifier, &authority],
        blockhash,
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
