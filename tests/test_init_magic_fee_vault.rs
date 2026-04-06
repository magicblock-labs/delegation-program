use dlp::solana_program;
use dlp_api::pda::magic_fee_vault_pda_from_validator;
use solana_program::{hash::Hash, native_token::LAMPORTS_PER_SOL};
use solana_program_test::{BanksClient, BanksClientError, ProgramTest};
use solana_sdk::{
    account::Account,
    instruction::InstructionError,
    signature::{Keypair, Signer},
    transaction::{Transaction, TransactionError},
};
use solana_sdk_ids::system_program;

use crate::fixtures::TEST_AUTHORITY;

mod fixtures;

#[tokio::test]
async fn test_init_magic_fee_vault() {
    let (banks, payer, admin, validator, blockhash) =
        setup_program_test_env().await;

    // Initialize the validator fees vault first (prerequisite)
    let ix = dlp_api::instruction_builder::init_validator_fees_vault(
        payer.pubkey(),
        admin.pubkey(),
        validator.pubkey(),
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &admin],
        blockhash,
    );
    banks.process_transaction(tx).await.unwrap();

    // Initialize the magic fee vault
    let ix = dlp_api::instruction_builder::init_magic_fee_vault(
        payer.pubkey(),
        validator.pubkey(),
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &validator],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok());

    // Assert the magic fee vault was created
    let magic_fee_vault_pda =
        magic_fee_vault_pda_from_validator(&validator.pubkey());
    let magic_fee_vault_account =
        banks.get_account(magic_fee_vault_pda).await.unwrap();
    assert!(magic_fee_vault_account.is_some());
}

#[tokio::test]
async fn test_init_magic_fee_vault_fails_without_validator_fees_vault() {
    let (banks, payer, _admin, validator, blockhash) =
        setup_program_test_env().await;

    // Skip initializing the validator fees vault — should fail
    let ix = dlp_api::instruction_builder::init_magic_fee_vault(
        payer.pubkey(),
        validator.pubkey(),
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &validator],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    assert!(
        matches!(
            res.unwrap_err(),
            BanksClientError::TransactionError(
                TransactionError::InstructionError(
                    _,
                    InstructionError::InvalidAccountOwner,
                )
            )
        ),
        "expected InvalidAccountOwner (validator fees vault not initialized)"
    );
}

async fn setup_program_test_env(
) -> (BanksClient, Keypair, Keypair, Keypair, Hash) {
    let mut program_test = ProgramTest::new("dlp", dlp_api::ID, None);
    program_test.prefer_bpf(true);

    let admin_keypair = crate::fixtures::keypair_from_bytes(&TEST_AUTHORITY);
    program_test.add_account(
        admin_keypair.pubkey(),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let validator_keypair = Keypair::new();
    program_test.add_account(
        validator_keypair.pubkey(),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks, payer, blockhash) = program_test.start().await;
    (banks, payer, admin_keypair, validator_keypair, blockhash)
}
