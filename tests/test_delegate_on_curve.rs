use solana_program::{hash::Hash, native_token::LAMPORTS_PER_SOL, system_program};
use solana_program_test::{processor, BanksClient, ProgramTest};
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

use crate::fixtures::ON_CURVE_KEYPAIR;
use dlp::args::DelegateArgs;
use dlp::pda::{
    delegate_buffer_pda_from_delegated_account_and_owner_program,
    delegation_metadata_pda_from_delegated_account, delegation_record_pda_from_delegated_account,
};
use dlp::state::{DelegationMetadata, DelegationRecord};

mod fixtures;

#[tokio::test]
async fn test_delegate_on_curve() {
    // Setup
    let (banks, payer, alt_payer, blockhash) = setup_program_test_env().await;

    // Save the PDA before delegation
    let delegated_account = alt_payer.pubkey();

    // Create transaction to change the owner of alt_payer
    let change_owner_ix =
        solana_program::system_instruction::assign(&alt_payer.pubkey(), &dlp::id());

    let change_owner_tx = Transaction::new_signed_with_payer(
        &[change_owner_ix],
        Some(&alt_payer.pubkey()),
        &[&alt_payer],
        blockhash,
    );

    // Process the transaction
    let change_owner_res = banks.process_transaction(change_owner_tx).await;
    assert!(change_owner_res.is_ok());

    // Verify the owner change
    let updated_alt_payer_account = banks
        .get_account(alt_payer.pubkey())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated_alt_payer_account.owner, dlp::id());

    // Submit the delegate tx
    let ix = dlp::instruction_builder::delegate(
        payer.pubkey(),
        delegated_account,
        None,
        DelegateArgs {
            commit_frequency_ms: u32::MAX,
            seeds: vec![],
            validator: Some(alt_payer.pubkey()),
        },
    );

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &alt_payer],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;

    println!("{:?}", res);
    assert!(res.is_ok());

    // Assert the buffer doesn't exist
    let delegate_buffer_pda = delegate_buffer_pda_from_delegated_account_and_owner_program(
        &delegated_account,
        &system_program::id(),
    );
    let buffer_account = banks.get_account(delegate_buffer_pda).await.unwrap();
    assert!(buffer_account.is_none());

    // Assert the PDA was delegated => owner is set to the delegation program
    let pda_account = banks.get_account(delegated_account).await.unwrap().unwrap();
    assert!(pda_account.owner.eq(&dlp::id()));

    // Assert that the PDA seeds account exists
    let delegation_metadata_pda =
        delegation_metadata_pda_from_delegated_account(&delegated_account);
    let delegation_metadata_account = banks
        .get_account(delegation_metadata_pda)
        .await
        .unwrap()
        .unwrap();
    assert!(delegation_metadata_account.owner.eq(&dlp::id()));

    // Assert that the delegation record exists and can be parsed
    let delegation_record_account = banks
        .get_account(delegation_record_pda_from_delegated_account(
            &delegated_account,
        ))
        .await
        .unwrap()
        .unwrap();
    let delegation_record =
        DelegationRecord::try_from_bytes_with_discriminator(&delegation_record_account.data)
            .unwrap();
    assert_eq!(delegation_record.owner, system_program::id());
    assert_eq!(delegation_record.authority, alt_payer.pubkey());

    // Assert that the delegation metadata exists and can be parsed
    let delegation_metadata = banks
        .get_account(delegation_metadata_pda_from_delegated_account(
            &delegated_account,
        ))
        .await
        .unwrap()
        .unwrap();
    assert!(delegation_metadata.owner.eq(&dlp::id()));
    let delegation_metadata =
        DelegationMetadata::try_from_bytes_with_discriminator(&delegation_metadata.data).unwrap();
    assert!(!delegation_metadata.is_undelegatable);
}

async fn setup_program_test_env() -> (BanksClient, Keypair, Keypair, Hash) {
    let mut program_test = ProgramTest::new("dlp", dlp::ID, processor!(dlp::process_instruction));
    program_test.prefer_bpf(true);
    let payer_alt = Keypair::from_bytes(&ON_CURVE_KEYPAIR).unwrap();

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

    let (banks, payer, blockhash) = program_test.start().await;
    (banks, payer, payer_alt, blockhash)
}
