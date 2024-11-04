use borsh::BorshDeserialize;
use solana_program::pubkey::Pubkey;
use solana_program::{hash::Hash, native_token::LAMPORTS_PER_SOL, system_program};
use solana_program_test::{processor, BanksClient, ProgramTest};
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

use crate::fixtures::ON_CURVE_KEYPAIR;
use dlp::consts::BUFFER;
use dlp::instruction::DelegateAccountArgs;
use dlp::pda::{delegation_metadata_pda_from_pubkey, delegation_record_pda_from_pubkey};
use dlp::state::{DelegationMetadata, DelegationRecord};
use dlp::utils::utils_account::AccountDeserialize;

mod fixtures;

#[tokio::test]
async fn test_delegate_on_curve() {
    // Setup
    let (mut banks, payer, alt_payer, blockhash) = setup_program_test_env().await;

    // Save the PDA before delegation
    let accounts_to_delegate = alt_payer.pubkey();

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
    let ix = dlp::instruction::delegate_on_curve(
        payer.pubkey(),
        accounts_to_delegate,
        system_program::id(),
        DelegateAccountArgs {
            valid_until: 0,
            commit_frequency_ms: u32::MAX,
            seeds: vec![],
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
    let buffer_pda = Pubkey::find_program_address(
        &[BUFFER, &accounts_to_delegate.to_bytes()],
        &system_program::id(),
    );
    let buffer_account = banks.get_account(buffer_pda.0).await.unwrap();
    assert!(buffer_account.is_none());

    // Assert the PDA was delegated => owner is set to the delegation program
    let pda_account = banks
        .get_account(accounts_to_delegate)
        .await
        .unwrap()
        .unwrap();
    assert!(pda_account.owner.eq(&dlp::id()));

    // Assert that the PDA seeds account exists
    let seeds_pda = delegation_metadata_pda_from_pubkey(&accounts_to_delegate);
    let pda_account = banks.get_account(seeds_pda).await.unwrap().unwrap();
    assert!(pda_account.owner.eq(&dlp::id()));

    // Assert that the delegation record exists and can be parsed
    let delegation_record = banks
        .get_account(delegation_record_pda_from_pubkey(&accounts_to_delegate))
        .await
        .unwrap()
        .unwrap();
    let delegation_record = DelegationRecord::try_from_bytes(&delegation_record.data).unwrap();
    assert_eq!(delegation_record.owner, system_program::id());

    // Assert that the delegation metadata exists and can be parsed
    let delegation_metadata = banks
        .get_account(delegation_metadata_pda_from_pubkey(&accounts_to_delegate))
        .await
        .unwrap()
        .unwrap();
    assert!(delegation_metadata.owner.eq(&dlp::id()));
    let delegation_metadata =
        DelegationMetadata::try_from_slice(&delegation_metadata.data).unwrap();
    assert_eq!(delegation_metadata.is_undelegatable, false);
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
