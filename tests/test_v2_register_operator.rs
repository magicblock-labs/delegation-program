use dlp_api::v2::{
    instruction_builder::register_operator, pda::operator_bond_pda,
    DlpV2Instruction, OperatorBond, RegisterOperatorArgs,
    OPERATOR_STATUS_ACTIVE,
};
use solana_program::native_token::LAMPORTS_PER_SOL;
use solana_program_test::ProgramTestBanksClientExt;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use solana_system_interface::instruction as system_instruction;
use wheels::layout::{Decodable, Encodable};

mod fixtures;

use crate::fixtures::v2::{init_v2, setup_program_test_env, valid_args};

#[test]
fn test_v2_register_operator_instruction_data_uses_one_byte_tag() {
    let args = RegisterOperatorArgs {
        amount_lamports: 42,
    };
    let ix = register_operator(
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        args.clone(),
    );
    let encoded_args = args.encode().unwrap();

    assert_eq!(ix.data[0], DlpV2Instruction::RegisterOperator as u8);
    assert_eq!(ix.data.len(), 1 + encoded_args.len());
    assert_eq!(&ix.data[1..], encoded_args.as_slice());

    let decoded =
        <RegisterOperatorArgs as Decodable>::decode(&ix.data[1..]).unwrap();
    assert_eq!(decoded.amount_lamports(), args.amount_lamports);
}

#[tokio::test]
async fn test_v2_register_operator() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;
    let config_args = valid_args();
    let operator = Keypair::new();

    init_v2(&banks, &payer, &authority, blockhash, config_args.clone()).await;
    fund_operator(&banks, &payer, &operator).await;

    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let ix = register_operator(
        operator.pubkey(),
        authority.pubkey(),
        RegisterOperatorArgs {
            amount_lamports: config_args.min_operator_bond,
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
        .get_account(operator_bond_pda(&operator.pubkey()))
        .await
        .unwrap()
        .unwrap();
    let operator_bond =
        <OperatorBond as Decodable>::decode(&operator_bond_account.data)
            .unwrap();

    assert_eq!(operator_bond.discriminator(), OperatorBond::DISCRIMINATOR);
    assert_eq!(*operator_bond.operator_identity(), operator.pubkey());
    assert_eq!(
        operator_bond.stake_lamports(),
        config_args.min_operator_bond
    );
    assert_eq!(operator_bond.locked_lamports(), 0);
    assert_eq!(operator_bond.status(), OPERATOR_STATUS_ACTIVE);
    assert_eq!(operator_bond.withdraw_requested_slot(), None);
    assert!(
        operator_bond_account.lamports > operator_bond.stake_lamports(),
        "operator bond account should hold rent plus stake"
    );
}

#[tokio::test]
async fn test_v2_register_operator_fails_with_wrong_authority() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;
    let config_args = valid_args();
    let operator = Keypair::new();

    init_v2(&banks, &payer, &authority, blockhash, config_args.clone()).await;
    fund_operator(&banks, &payer, &operator).await;

    let wrong_authority = Keypair::new();
    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let ix = register_operator(
        operator.pubkey(),
        wrong_authority.pubkey(),
        RegisterOperatorArgs {
            amount_lamports: config_args.min_operator_bond,
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
async fn test_v2_register_operator_fails_with_low_stake() {
    let (banks, payer, authority, blockhash) = setup_program_test_env().await;
    let config_args = valid_args();
    let operator = Keypair::new();

    init_v2(&banks, &payer, &authority, blockhash, config_args).await;
    fund_operator(&banks, &payer, &operator).await;

    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let ix = register_operator(
        operator.pubkey(),
        authority.pubkey(),
        RegisterOperatorArgs { amount_lamports: 0 },
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
async fn test_v2_register_operator_fails_twice() {
    let (mut banks, payer, authority, blockhash) =
        setup_program_test_env().await;
    let config_args = valid_args();
    let operator = Keypair::new();

    init_v2(&banks, &payer, &authority, blockhash, config_args.clone()).await;
    fund_operator(&banks, &payer, &operator).await;

    let ix = register_operator(
        operator.pubkey(),
        authority.pubkey(),
        RegisterOperatorArgs {
            amount_lamports: config_args.min_operator_bond,
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

    let ix = register_operator(
        operator.pubkey(),
        authority.pubkey(),
        RegisterOperatorArgs {
            amount_lamports: config_args.min_operator_bond,
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
