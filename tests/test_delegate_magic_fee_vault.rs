use dlp::pda::{
    delegation_record_pda_from_delegated_account,
    magic_fee_vault_pda_from_validator,
};
use solana_program::{
    hash::Hash, native_token::LAMPORTS_PER_SOL, system_program,
};
use solana_program_test::{BanksClient, BanksClientError, ProgramTest};
use solana_sdk::{
    account::Account,
    instruction::InstructionError,
    signature::{Keypair, Signer},
    transaction::{Transaction, TransactionError},
};

use crate::fixtures::TEST_AUTHORITY;

mod fixtures;

/// Helper: init validator fees vault + magic fee vault for a given validator.
async fn setup_vault(
    banks: &BanksClient,
    payer: &Keypair,
    admin: &Keypair,
    validator: &Keypair,
    blockhash: Hash,
) {
    let ix = dlp_api::instruction_builder::init_validator_fees_vault(
        payer.pubkey(),
        admin.pubkey(),
        validator.pubkey(),
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer, admin],
        blockhash,
    );
    banks.process_transaction(tx).await.unwrap();

    let ix = dlp_api::instruction_builder::init_magic_fee_vault(
        payer.pubkey(),
        validator.pubkey(),
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer, validator],
        blockhash,
    );
    banks.process_transaction(tx).await.unwrap();
}

#[tokio::test]
async fn test_delegate_magic_fee_vault() {
    let (banks, payer, admin, validator, blockhash) =
        setup_program_test_env().await;

    setup_vault(&banks, &payer, &admin, &validator, blockhash).await;

    // Delegate the magic fee vault
    let ix = dlp_api::instruction_builder::delegate_magic_fee_vault(
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
    assert!(res.is_ok(), "delegate_magic_fee_vault failed: {:?}", res);

    // Assert delegation record was created
    let magic_fee_vault =
        magic_fee_vault_pda_from_validator(&validator.pubkey());
    let delegation_record =
        delegation_record_pda_from_delegated_account(&magic_fee_vault);
    let record_account = banks.get_account(delegation_record).await.unwrap();
    assert!(record_account.is_some(), "delegation record should exist");
}

#[tokio::test]
async fn test_delegate_magic_fee_vault_fails_without_fees_vault() {
    let (banks, payer, _admin, validator, blockhash) =
        setup_program_test_env().await;

    // No validator fees vault or magic fee vault initialized — should fail
    let ix = dlp_api::instruction_builder::delegate_magic_fee_vault(
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

#[tokio::test]
async fn test_delegate_magic_fee_vault_fails_without_magic_fee_vault() {
    let (banks, payer, admin, validator, blockhash) =
        setup_program_test_env().await;

    // Init validator fees vault but NOT the magic fee vault
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

    let ix = dlp_api::instruction_builder::delegate_magic_fee_vault(
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
        "expected InvalidAccountOwner (magic fee vault not initialized)"
    );
}

#[tokio::test]
async fn test_delegate_magic_fee_vault_fails_with_wrong_validator() {
    let (banks, payer, admin, validator, blockhash) =
        setup_program_test_env().await;

    setup_vault(&banks, &payer, &admin, &validator, blockhash).await;

    // A different validator tries to delegate the vault
    let wrong_validator = Keypair::new();
    let ix = dlp_api::instruction_builder::delegate_magic_fee_vault(
        payer.pubkey(),
        wrong_validator.pubkey(),
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &wrong_validator],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    // wrong_validator has no fees vault → load_initialized_validator_fees_vault
    // hits load_owned_pda on a system-owned account → InvalidAccountOwner
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
        "expected InvalidAccountOwner (wrong validator has no fees vault)"
    );
}

async fn setup_program_test_env(
) -> (BanksClient, Keypair, Keypair, Keypair, Hash) {
    let mut program_test = ProgramTest::new("dlp", dlp::ID, None);
    program_test.prefer_bpf(true);

    let admin_keypair = Keypair::from_bytes(&TEST_AUTHORITY).unwrap();
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
