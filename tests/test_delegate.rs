use solana_program::pubkey::Pubkey;
use solana_program::rent::Rent;
use solana_program::{hash::Hash, native_token::LAMPORTS_PER_SOL, system_program};
use solana_program_test::{processor, read_file, BanksClient, ProgramTest};
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

use dlp::consts::BUFFER;
use dlp::pda::{
    delegation_metadata_pda_from_delegated_account, delegation_record_pda_from_delegated_account,
};
use dlp::state::DelegationRecord;

use crate::fixtures::{
    DELEGATED_PDA_ID, DELEGATED_PDA_OWNER_ID, EXTERNAL_DELEGATE_INSTRUCTION_DISCRIMINATOR,
};

mod fixtures;

#[tokio::test]
async fn test_delegate() {
    // Setup
    let (mut banks, payer, _, blockhash) = setup_program_test_env().await;

    // Save the PDA before delegation
    let pda_before_delegation = banks.get_account(DELEGATED_PDA_ID).await.unwrap().unwrap();
    let pda_data_before_delegation = pda_before_delegation.data.clone();

    // Submit the delegate tx
    let ix = delegate_from_wrapper_program(payer.pubkey(), DELEGATED_PDA_ID);

    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[&payer], blockhash);
    let res = banks.process_transaction(tx).await;

    println!("{:?}", res);
    assert!(res.is_ok());

    // Assert the buffer was closed
    let buffer_pda = Pubkey::find_program_address(
        &[BUFFER, &DELEGATED_PDA_ID.to_bytes()],
        &DELEGATED_PDA_OWNER_ID,
    );
    let buffer_account = banks.get_account(buffer_pda.0).await.unwrap();
    assert!(buffer_account.is_none());

    // Assert the PDA was delegated => owner is set to the delegation program
    let pda_account = banks.get_account(DELEGATED_PDA_ID).await.unwrap().unwrap();
    assert!(pda_account.owner.eq(&dlp::id()));

    // Assert the PDA data was not changed
    assert_eq!(pda_data_before_delegation, pda_account.data);

    // Assert that the PDA seeds account exists
    let delegation_metadata_pda = delegation_metadata_pda_from_delegated_account(&DELEGATED_PDA_ID);
    let delegation_metadata_account = banks
        .get_account(delegation_metadata_pda)
        .await
        .unwrap()
        .unwrap();
    assert!(delegation_metadata_account.owner.eq(&dlp::id()));

    // Assert that the delegation record exists and can be parsed
    let delegation_record = banks
        .get_account(delegation_record_pda_from_delegated_account(
            &DELEGATED_PDA_ID,
        ))
        .await
        .unwrap()
        .unwrap();
    let delegation_record =
        DelegationRecord::try_from_bytes_with_discriminator(&delegation_record.data).unwrap();
    assert_eq!(delegation_record.owner, DELEGATED_PDA_OWNER_ID);
}

async fn setup_program_test_env() -> (BanksClient, Keypair, Keypair, Hash) {
    let mut program_test = ProgramTest::new("dlp", dlp::ID, processor!(dlp::process_instruction));
    program_test.prefer_bpf(true);
    let payer_alt = Keypair::new();

    program_test.add_account(
        payer_alt.pubkey(),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup a PDA
    program_test.add_account(
        DELEGATED_PDA_ID,
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
            owner: DELEGATED_PDA_OWNER_ID,
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup program to test delegation
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

    let (banks, payer, blockhash) = program_test.start().await;
    (banks, payer, payer_alt, blockhash)
}

/// Builds a delegate instruction for the test program
fn delegate_from_wrapper_program(payer: Pubkey, delegate_account: Pubkey) -> Instruction {
    let buffer = Pubkey::find_program_address(
        &[BUFFER, &delegate_account.to_bytes()],
        &DELEGATED_PDA_OWNER_ID,
    );
    let delegation_record_pda = delegation_record_pda_from_delegated_account(&delegate_account);
    let delegation_metadata_pda = delegation_metadata_pda_from_delegated_account(&delegate_account);
    Instruction {
        program_id: DELEGATED_PDA_OWNER_ID,
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(buffer.0, false),
            AccountMeta::new(delegation_record_pda, false),
            AccountMeta::new(delegation_metadata_pda, false),
            AccountMeta::new(delegate_account, false),
            AccountMeta::new_readonly(DELEGATED_PDA_OWNER_ID, false),
            AccountMeta::new_readonly(dlp::id(), false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: EXTERNAL_DELEGATE_INSTRUCTION_DISCRIMINATOR.to_vec(),
    }
}
