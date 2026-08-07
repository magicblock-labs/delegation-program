use dlp_api::v2::{
    instruction_builder::register_operator, pda::operator_bond_pda,
    OperatorBond, RegisterOperatorArgs, OPERATOR_STATUS_ACTIVE,
};
use solana_sdk::{
    signature::{Keypair, Signer},
    transaction::Transaction,
};

mod fixtures;

use crate::fixtures::v2::{init_v2, setup_program_test_env, valid_args};

#[tokio::test]
async fn test_v2_register_operator() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;
    let config_args = valid_args();

    init_v2(&banks, &payer, &authority, blockhash, config_args.clone()).await;

    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let ix = register_operator(
        payer.pubkey(),
        authority.pubkey(),
        RegisterOperatorArgs {
            amount_lamports: config_args.min_operator_bond,
        },
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &authority],
        blockhash,
    );

    assert!(banks.process_transaction(tx).await.is_ok());

    let operator_bond_account = banks
        .get_account(operator_bond_pda(&payer.pubkey()))
        .await
        .unwrap()
        .unwrap();
    let operator_bond = OperatorBond::try_from_bytes_with_discriminator(
        &operator_bond_account.data,
    )
    .unwrap();

    assert_eq!(operator_bond.operator_identity, payer.pubkey());
    assert_eq!(operator_bond.stake_lamports, config_args.min_operator_bond);
    assert_eq!(operator_bond.locked_lamports, 0);
    assert_eq!(operator_bond.status, OPERATOR_STATUS_ACTIVE);
    assert_eq!(operator_bond.withdraw_requested_slot, None);
    assert!(
        operator_bond_account.lamports > operator_bond.stake_lamports,
        "operator bond account should hold rent plus stake"
    );
}

#[tokio::test]
async fn test_v2_register_operator_fails_with_wrong_authority() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;
    let config_args = valid_args();

    init_v2(&banks, &payer, &authority, blockhash, config_args.clone()).await;

    let wrong_authority = Keypair::new();
    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let ix = register_operator(
        payer.pubkey(),
        wrong_authority.pubkey(),
        RegisterOperatorArgs {
            amount_lamports: config_args.min_operator_bond,
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
async fn test_v2_register_operator_fails_with_low_stake() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;
    let config_args = valid_args();

    init_v2(&banks, &payer, &authority, blockhash, config_args).await;

    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let ix = register_operator(
        payer.pubkey(),
        authority.pubkey(),
        RegisterOperatorArgs { amount_lamports: 0 },
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
async fn test_v2_register_operator_fails_twice() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;
    let config_args = valid_args();

    init_v2(&banks, &payer, &authority, blockhash, config_args.clone()).await;

    let ix = register_operator(
        payer.pubkey(),
        authority.pubkey(),
        RegisterOperatorArgs {
            amount_lamports: config_args.min_operator_bond,
        },
    );
    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &authority],
        blockhash,
    );
    banks.process_transaction(tx).await.unwrap();

    let ix = register_operator(
        payer.pubkey(),
        authority.pubkey(),
        RegisterOperatorArgs {
            amount_lamports: config_args.min_operator_bond,
        },
    );
    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &authority],
        blockhash,
    );

    assert!(banks.process_transaction(tx).await.is_err());
}
