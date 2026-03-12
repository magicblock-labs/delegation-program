use dlp::{
    args::DelegateArgs, error::DlpError,
    pda::delegation_record_pda_from_delegated_account, state::DelegationRecord,
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

use crate::fixtures::ON_CURVE_KEYPAIR;

mod fixtures;

#[tokio::test]
async fn test_delegation_confined_accounts_rejects_system_validator() {
    let (banks, payer, delegated, blockhash) = setup_program_test_env().await;

    let assign_ix = solana_program::system_instruction::assign(
        &delegated.pubkey(),
        &dlp::id(),
    );
    let assign_tx = Transaction::new_signed_with_payer(
        &[assign_ix],
        Some(&delegated.pubkey()),
        &[&delegated],
        blockhash,
    );
    let assign_res = banks.process_transaction(assign_tx).await;
    assert!(assign_res.is_ok());

    let ix = dlp_api::instruction_builder::delegate(
        payer.pubkey(),
        delegated.pubkey(),
        None,
        DelegateArgs {
            commit_frequency_ms: 0,
            seeds: vec![],
            validator: Some(system_program::id()),
        },
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &delegated],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    let err = res.unwrap_err();
    match err {
        BanksClientError::TransactionError(
            TransactionError::InstructionError(
                _,
                InstructionError::Custom(code),
            ),
        ) => {
            assert_eq!(
                code,
                DlpError::DelegationToSystemProgramNotAllowed as u32
            );
        }
        _ => panic!("unexpected error: {err:?}"),
    }
}

#[tokio::test]
async fn test_delegation_confined_accounts_allows_system_validator() {
    let (banks, payer, delegated, blockhash) = setup_program_test_env().await;

    let assign_ix = solana_program::system_instruction::assign(
        &delegated.pubkey(),
        &dlp::id(),
    );
    let assign_tx = Transaction::new_signed_with_payer(
        &[assign_ix],
        Some(&delegated.pubkey()),
        &[&delegated],
        blockhash,
    );
    let assign_res = banks.process_transaction(assign_tx).await;
    assert!(assign_res.is_ok());

    let ix = dlp_api::instruction_builder::delegate_with_any_validator(
        payer.pubkey(),
        delegated.pubkey(),
        None,
        DelegateArgs {
            commit_frequency_ms: 0,
            seeds: vec![],
            validator: Some(system_program::id()),
        },
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &delegated],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok());

    let delegation_record_account = banks
        .get_account(delegation_record_pda_from_delegated_account(
            &delegated.pubkey(),
        ))
        .await
        .expect("failed to fetch delegation record account")
        .expect("delegation record account missing for delegated pubkey");
    let delegation_record =
        DelegationRecord::try_from_bytes_with_discriminator(
            &delegation_record_account.data,
        )
        .expect("failed to deserialize DelegationRecord for delegated pubkey");
    assert_eq!(delegation_record.authority, system_program::id());
}

async fn setup_program_test_env() -> (BanksClient, Keypair, Keypair, Hash) {
    let mut program_test = ProgramTest::new("dlp", dlp::ID, None);
    program_test.prefer_bpf(true);
    let delegated = Keypair::from_bytes(&ON_CURVE_KEYPAIR).unwrap();

    program_test.add_account(
        delegated.pubkey(),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks, payer, blockhash) = program_test.start().await;
    (banks, payer, delegated, blockhash)
}
