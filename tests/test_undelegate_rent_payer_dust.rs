use dlp::solana_program;
use dlp_api::{
    consts::SESSION_FEE_LAMPORTS,
    pda::{
        delegation_metadata_pda_from_delegated_account,
        delegation_record_pda_from_delegated_account, fees_vault_pda,
        validator_fees_vault_pda_from_validator,
    },
};
use solana_program::{
    hash::Hash, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey, rent::Rent,
};
use solana_program_test::{read_file, BanksClient, ProgramTest};
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use solana_sdk_ids::system_program;

use crate::fixtures::{
    create_delegation_metadata_data_with_commit_id, get_delegation_record_data,
    DELEGATED_PDA_ID, DELEGATED_PDA_OWNER_ID, TEST_AUTHORITY,
};

mod fixtures;

/// A single 32-byte seed makes the metadata PDA large enough that the
/// delegation rent exceeds the session fee by a few thousand lamports.
const RENT_PAYER_SEED: [u8; 32] = [7; 32];

/// Mirrors `process_delegation_cleanup`: the fee is taken from the record
/// first, then from the metadata, and whatever is left is the refund.
struct ExpectedCleanup {
    protocol_fee: u64,
    validator_fee: u64,
    refund: u64,
}

fn expected_cleanup(record_rent: u64, metadata_rent: u64) -> ExpectedCleanup {
    let mut fee_remaining = SESSION_FEE_LAMPORTS;
    let record_fee = fee_remaining.min(record_rent);
    fee_remaining -= record_fee;
    let metadata_fee = fee_remaining.min(metadata_rent);
    let protocol_fee = record_fee / 10 + metadata_fee / 10;
    ExpectedCleanup {
        protocol_fee,
        validator_fee: record_fee + metadata_fee - protocol_fee,
        refund: record_rent + metadata_rent - record_fee - metadata_fee,
    }
}

#[tokio::test]
async fn test_undelegate_routes_dust_refund_to_validator_vault_when_rent_payer_is_empty(
) {
    let rent_payer = Pubkey::new_unique();
    let (banks, validator, blockhash, record_rent, metadata_rent) =
        setup_program_test_env(rent_payer, false).await;
    let expected = expected_cleanup(record_rent, metadata_rent);
    let rent_exempt_min = Rent::default().minimum_balance(0);
    assert!(expected.refund > 0 && expected.refund < rent_exempt_min);

    let fees_vault_before = lamports(&banks, fees_vault_pda()).await;
    let validator_vault_pda =
        validator_fees_vault_pda_from_validator(&validator.pubkey());
    let validator_vault_before = lamports(&banks, validator_vault_pda).await;

    let ix = dlp_api::instruction_builder::undelegate(
        validator.pubkey(),
        DELEGATED_PDA_ID,
        DELEGATED_PDA_OWNER_ID,
        rent_payer,
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&validator.pubkey()),
        &[&validator],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok(), "{res:?}");

    // The empty rent payer must not be left rent-paying, so it stays empty
    assert!(banks.get_account(rent_payer).await.unwrap().is_none());

    // The dust that could not be refunded goes to the validator vault
    assert_eq!(
        lamports(&banks, fees_vault_pda()).await,
        fees_vault_before + expected.protocol_fee
    );
    assert_eq!(
        lamports(&banks, validator_vault_pda).await,
        validator_vault_before + expected.validator_fee + expected.refund
    );
    assert!(banks.get_account(DELEGATED_PDA_ID).await.unwrap().is_some());
}

#[tokio::test]
async fn test_undelegate_still_refunds_dust_to_funded_rent_payer() {
    let rent_payer = Pubkey::new_unique();
    let (banks, validator, blockhash, record_rent, metadata_rent) =
        setup_program_test_env(rent_payer, true).await;
    let expected = expected_cleanup(record_rent, metadata_rent);

    let rent_payer_before = lamports(&banks, rent_payer).await;
    let fees_vault_before = lamports(&banks, fees_vault_pda()).await;
    let validator_vault_pda =
        validator_fees_vault_pda_from_validator(&validator.pubkey());
    let validator_vault_before = lamports(&banks, validator_vault_pda).await;

    let ix = dlp_api::instruction_builder::undelegate(
        validator.pubkey(),
        DELEGATED_PDA_ID,
        DELEGATED_PDA_OWNER_ID,
        rent_payer,
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&validator.pubkey()),
        &[&validator],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok(), "{res:?}");

    assert_eq!(
        lamports(&banks, rent_payer).await,
        rent_payer_before + expected.refund
    );
    assert_eq!(
        lamports(&banks, fees_vault_pda()).await,
        fees_vault_before + expected.protocol_fee
    );
    assert_eq!(
        lamports(&banks, validator_vault_pda).await,
        validator_vault_before + expected.validator_fee
    );
}

async fn lamports(banks: &BanksClient, pubkey: Pubkey) -> u64 {
    banks.get_account(pubkey).await.unwrap().unwrap().lamports
}

async fn setup_program_test_env(
    rent_payer: Pubkey,
    fund_rent_payer: bool,
) -> (BanksClient, Keypair, Hash, u64, u64) {
    let mut program_test = ProgramTest::new("dlp", dlp_api::ID, None);
    program_test.prefer_bpf(true);
    let validator = crate::fixtures::keypair_from_bytes(&TEST_AUTHORITY);

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

    if fund_rent_payer {
        program_test.add_account(
            rent_payer,
            Account {
                lamports: LAMPORTS_PER_SOL,
                data: vec![],
                owner: system_program::id(),
                executable: false,
                rent_epoch: 0,
            },
        );
    }

    // Setup a delegated PDA (no data, so undelegation takes the fast path)
    program_test.add_account(
        DELEGATED_PDA_ID,
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: dlp_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the delegation record PDA
    let delegation_record_data =
        get_delegation_record_data(validator.pubkey(), None);
    let record_rent =
        Rent::default().minimum_balance(delegation_record_data.len());
    program_test.add_account(
        delegation_record_pda_from_delegated_account(&DELEGATED_PDA_ID),
        Account {
            lamports: record_rent,
            data: delegation_record_data,
            owner: dlp_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the delegation metadata PDA: first commit of the session, so the
    // fee is exactly the session fee and the rent leaves a small refund
    let delegation_metadata_data =
        create_delegation_metadata_data_with_commit_id(
            rent_payer,
            &[&RENT_PAYER_SEED],
            true,
            1,
        );
    let metadata_rent =
        Rent::default().minimum_balance(delegation_metadata_data.len());
    program_test.add_account(
        delegation_metadata_pda_from_delegated_account(&DELEGATED_PDA_ID),
        Account {
            lamports: metadata_rent,
            data: delegation_metadata_data,
            owner: dlp_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the owner program
    let data = read_file("tests/buffers/test_delegation.so");
    program_test.add_account(
        DELEGATED_PDA_OWNER_ID,
        Account {
            lamports: Rent::default().minimum_balance(data.len()).max(1),
            data,
            owner: solana_sdk::bpf_loader::id(),
            executable: true,
            rent_epoch: 0,
        },
    );

    // Setup the protocol fees vault
    program_test.add_account(
        fees_vault_pda(),
        Account {
            lamports: Rent::default().minimum_balance(0),
            data: vec![],
            owner: dlp_api::id(),
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
            owner: dlp_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks, _, blockhash) = program_test.start().await;
    (banks, validator, blockhash, record_rent, metadata_rent)
}
