use dlp_api::v2::{
    instruction_builder::register_operator, pda::OPERATOR_BOND_SEED,
    OperatorBond, OperatorStatus, RegisterOperatorArgs,
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
async fn test_register_operator() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;
    let operator = Keypair::new();
    let cluster_args = challenge_cluster_args(operator.pubkey());
    let min_operator_bond = cluster_args
        .challenge_config
        .as_ref()
        .unwrap()
        .min_operator_bond;
    let delegation_cluster =
        dlp_api::v2::pda::delegation_cluster_pda(&cluster_args.cluster_id);
    let (operator_bond_address, expected_operator_bond_bump) =
        Pubkey::find_program_address(
            &[OPERATOR_BOND_SEED, delegation_cluster.as_ref()],
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
    fund_operator(&banks, &payer, &operator).await;

    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let ix = register_operator(
        operator.pubkey(),
        authority.pubkey(),
        delegation_cluster,
        RegisterOperatorArgs {
            stake_lamports: min_operator_bond,
        },
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &operator, &authority],
        blockhash,
    );

    assert!(banks.process_transaction(tx).await.is_ok());

    let operator_bond_account = banks
        .get_account(operator_bond_address)
        .await
        .unwrap()
        .unwrap();
    let operator_bond =
        OperatorBond::decode(&operator_bond_account.data).unwrap();
    let expected_operator_bond_lamports = Rent::default()
        .minimum_balance(OperatorBond::DATA_LEN)
        + min_operator_bond;

    assert_eq!(operator_bond.discriminator(), OperatorBond::DISCRIMINATOR);
    assert_eq!(operator_bond.bump(), expected_operator_bond_bump);
    assert_eq!(*operator_bond.delegation_cluster(), delegation_cluster);
    assert_eq!(operator_bond.stake_lamports(), min_operator_bond);
    assert_eq!(operator_bond.locked_lamports(), 0);
    assert_eq!(operator_bond.status(), OperatorStatus::Active.value());
    assert_eq!(operator_bond.withdraw_requested_slot(), None);
    assert_eq!(
        operator_bond_account.lamports,
        expected_operator_bond_lamports
    );
}

#[tokio::test]
async fn test_register_operator_fails_with_wrong_authority() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;
    let operator = Keypair::new();
    let cluster_args = challenge_cluster_args(operator.pubkey());
    let min_operator_bond = cluster_args
        .challenge_config
        .as_ref()
        .unwrap()
        .min_operator_bond;

    let delegation_cluster = initialize_delegation_cluster(
        &banks,
        &payer,
        &authority,
        blockhash,
        cluster_args,
    )
    .await;
    fund_operator(&banks, &payer, &operator).await;

    let wrong_authority = Keypair::new();
    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let ix = register_operator(
        operator.pubkey(),
        wrong_authority.pubkey(),
        delegation_cluster,
        RegisterOperatorArgs {
            stake_lamports: min_operator_bond,
        },
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &operator, &wrong_authority],
        blockhash,
    );

    assert!(banks.process_transaction(tx).await.is_err());
}

#[tokio::test]
async fn test_register_operator_fails_with_wrong_validator() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;
    let current_validator = Keypair::new();
    let wrong_validator = Keypair::new();
    let cluster_args = challenge_cluster_args(current_validator.pubkey());
    let min_operator_bond = cluster_args
        .challenge_config
        .as_ref()
        .unwrap()
        .min_operator_bond;
    let delegation_cluster = initialize_delegation_cluster(
        &banks,
        &payer,
        &authority,
        blockhash,
        cluster_args,
    )
    .await;
    fund_operator(&banks, &payer, &wrong_validator).await;

    let ix = register_operator(
        wrong_validator.pubkey(),
        authority.pubkey(),
        delegation_cluster,
        RegisterOperatorArgs {
            stake_lamports: min_operator_bond,
        },
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &wrong_validator, &authority],
        banks.get_latest_blockhash().await.unwrap(),
    );

    assert!(banks.process_transaction(tx).await.is_err());
}

#[tokio::test]
async fn test_register_operator_fails_with_low_stake() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;
    let operator = Keypair::new();
    let cluster_args = challenge_cluster_args(operator.pubkey());

    let delegation_cluster = initialize_delegation_cluster(
        &banks,
        &payer,
        &authority,
        blockhash,
        cluster_args,
    )
    .await;
    fund_operator(&banks, &payer, &operator).await;

    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let ix = register_operator(
        operator.pubkey(),
        authority.pubkey(),
        delegation_cluster,
        RegisterOperatorArgs { stake_lamports: 0 },
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &operator, &authority],
        blockhash,
    );

    assert!(banks.process_transaction(tx).await.is_err());
}

#[tokio::test]
async fn test_register_operator_fails_twice() {
    let (mut banks, payer, authority, blockhash) =
        setup_program_test_env().await;
    let operator = Keypair::new();
    let cluster_args = challenge_cluster_args(operator.pubkey());
    let min_operator_bond = cluster_args
        .challenge_config
        .as_ref()
        .unwrap()
        .min_operator_bond;

    let delegation_cluster = initialize_delegation_cluster(
        &banks,
        &payer,
        &authority,
        blockhash,
        cluster_args,
    )
    .await;
    fund_operator(&banks, &payer, &operator).await;

    {
        let ix = register_operator(
            operator.pubkey(),
            authority.pubkey(),
            delegation_cluster,
            RegisterOperatorArgs {
                stake_lamports: min_operator_bond,
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
            &[&payer, &operator, &authority],
            blockhash,
        );
        banks.process_transaction(tx).await.unwrap();
    }

    {
        let ix = register_operator(
            operator.pubkey(),
            authority.pubkey(),
            delegation_cluster,
            RegisterOperatorArgs {
                stake_lamports: min_operator_bond,
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
            &[&payer, &operator, &authority],
            blockhash,
        );

        assert!(banks.process_transaction(tx).await.is_err());
    }
}

#[tokio::test]
async fn test_register_operator_rejects_direct_cluster() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;
    let operator = Keypair::new();
    let delegation_cluster = initialize_delegation_cluster(
        &banks,
        &payer,
        &authority,
        blockhash,
        direct_cluster_args(operator.pubkey()),
    )
    .await;
    fund_operator(&banks, &payer, &operator).await;

    let ix = register_operator(
        operator.pubkey(),
        authority.pubkey(),
        delegation_cluster,
        RegisterOperatorArgs { stake_lamports: 1 },
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &operator, &authority],
        banks.get_latest_blockhash().await.unwrap(),
    );

    assert!(banks.process_transaction(tx).await.is_err());
}

async fn fund_operator(
    banks: &solana_program_test::BanksClient,
    payer: &Keypair,
    operator: &Keypair,
) {
    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let ix = system_instruction::transfer(
        &payer.pubkey(),
        &operator.pubkey(),
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
