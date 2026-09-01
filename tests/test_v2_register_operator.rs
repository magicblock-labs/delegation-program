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
    initialize_protocol_config, setup_program_test_env,
    valid_protocol_config_args,
};

#[tokio::test]
async fn test_register_operator() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;
    let config_args = valid_protocol_config_args();
    let operator = Keypair::new();
    let (operator_bond_address, expected_operator_bond_bump) =
        Pubkey::find_program_address(
            &[OPERATOR_BOND_SEED, operator.pubkey().as_ref()],
            &dlp_api::id(),
        );

    initialize_protocol_config(
        &banks,
        &payer,
        &authority,
        blockhash,
        config_args.clone(),
    )
    .await;
    fund_operator(&banks, &payer, &operator).await;

    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let ix = register_operator(
        operator.pubkey(),
        authority.pubkey(),
        RegisterOperatorArgs {
            stake_lamports: config_args.min_operator_bond,
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
        + config_args.min_operator_bond;

    assert_eq!(operator_bond.discriminator(), OperatorBond::DISCRIMINATOR);
    assert_eq!(operator_bond.bump(), expected_operator_bond_bump);
    assert_eq!(*operator_bond.operator_identity(), operator.pubkey());
    assert_eq!(
        operator_bond.stake_lamports(),
        config_args.min_operator_bond
    );
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
    let config_args = valid_protocol_config_args();
    let operator = Keypair::new();

    initialize_protocol_config(
        &banks,
        &payer,
        &authority,
        blockhash,
        config_args.clone(),
    )
    .await;
    fund_operator(&banks, &payer, &operator).await;

    let wrong_authority = Keypair::new();
    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let ix = register_operator(
        operator.pubkey(),
        wrong_authority.pubkey(),
        RegisterOperatorArgs {
            stake_lamports: config_args.min_operator_bond,
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
async fn test_register_operator_fails_with_low_stake() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;
    let config_args = valid_protocol_config_args();
    let operator = Keypair::new();

    initialize_protocol_config(
        &banks,
        &payer,
        &authority,
        blockhash,
        config_args,
    )
    .await;
    fund_operator(&banks, &payer, &operator).await;

    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let ix = register_operator(
        operator.pubkey(),
        authority.pubkey(),
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
    let config_args = valid_protocol_config_args();
    let operator = Keypair::new();

    initialize_protocol_config(
        &banks,
        &payer,
        &authority,
        blockhash,
        config_args.clone(),
    )
    .await;
    fund_operator(&banks, &payer, &operator).await;

    {
        let ix = register_operator(
            operator.pubkey(),
            authority.pubkey(),
            RegisterOperatorArgs {
                stake_lamports: config_args.min_operator_bond,
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
            RegisterOperatorArgs {
                stake_lamports: config_args.min_operator_bond,
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
