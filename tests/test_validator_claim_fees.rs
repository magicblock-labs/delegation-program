use crate::fixtures::TEST_AUTHORITY;
use dlp::consts::PROTOCOL_FEES_PERCENTAGE;
use dlp::pda::{fees_vault_pda, validator_fees_vault_pda_from_validator};
use solana_program::{hash::Hash, native_token::LAMPORTS_PER_SOL, system_program};
use solana_program_test::{processor, BanksClient, ProgramTest};
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

mod fixtures;

#[tokio::test]
async fn test_validator_claim_fees() {
    // Setup
    let (banks, payer, validator, blockhash) = setup_program_test_env().await;

    let fees_vault_pda = fees_vault_pda();
    let fees_vault_init_lamports = banks
        .get_account(fees_vault_pda)
        .await
        .unwrap()
        .unwrap()
        .lamports;

    let validator_fees_vault_pda = validator_fees_vault_pda_from_validator(&validator.pubkey());
    let validator_fees_vault_init_lamports = banks
        .get_account(validator_fees_vault_pda)
        .await
        .unwrap()
        .unwrap()
        .lamports;

    let validator_init_lamports = banks
        .get_account(validator.pubkey())
        .await
        .unwrap()
        .unwrap()
        .lamports;

    // Submit the undelegate tx
    let withdrawal_amount = 100000;
    let ix =
        dlp::instruction_builder::validator_claim_fees(validator.pubkey(), Some(withdrawal_amount));
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &validator],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok());

    // Assert the validator fees vault now has less lamports
    let validator_fees_vault_account = banks.get_account(validator_fees_vault_pda).await.unwrap();
    assert!(validator_fees_vault_account.is_some());
    assert_eq!(
        validator_fees_vault_account.unwrap().lamports,
        validator_fees_vault_init_lamports - withdrawal_amount
    );

    // Assert the fees vault now has prev lamports + fees
    let protocol_fees = (withdrawal_amount * u64::from(PROTOCOL_FEES_PERCENTAGE)) / 100;
    let fees_vault_account = banks.get_account(fees_vault_pda).await.unwrap();
    assert!(fees_vault_account.is_some());
    assert_eq!(
        fees_vault_account.unwrap().lamports,
        fees_vault_init_lamports + protocol_fees
    );

    let claim_amount = withdrawal_amount.saturating_sub(protocol_fees);
    let validator_account = banks.get_account(validator.pubkey()).await.unwrap();
    assert_eq!(
        validator_account.unwrap().lamports,
        validator_init_lamports + claim_amount
    );
}

async fn setup_program_test_env() -> (BanksClient, Keypair, Keypair, Hash) {
    let mut program_test = ProgramTest::new("dlp", dlp::ID, processor!(dlp::process_instruction));
    program_test.prefer_bpf(true);
    let validator = Keypair::from_bytes(&TEST_AUTHORITY).unwrap();

    program_test.add_account(
        validator.pubkey(),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the fees vault account
    program_test.add_account(
        fees_vault_pda(),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the validator fees vault
    program_test.add_account(
        validator_fees_vault_pda_from_validator(&validator.pubkey()),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks, payer, blockhash) = program_test.start().await;
    (banks, payer, validator, blockhash)
}
