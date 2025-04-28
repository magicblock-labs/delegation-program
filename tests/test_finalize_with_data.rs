use crate::fixtures::{create_delegation_metadata_data, create_delegation_record_data, get_commit_record_account_data, get_delegation_metadata_data, get_delegation_record_data, COMMIT_NEW_STATE_ACCOUNT_DATA, DELEGATED_PDA_ID, TEST_AUTHORITY};
use dlp::args::{DelegateEphemeralBalanceArgs, FinalizeWithDataArgs};
use dlp::pda::{
    commit_record_pda_from_delegated_account, commit_state_pda_from_delegated_account,
    delegation_metadata_pda_from_delegated_account, delegation_record_pda_from_delegated_account,
    ephemeral_balance_pda_from_payer, validator_fees_vault_pda_from_validator,
};
use dlp::state::{CommitRecord, DelegationMetadata};
use solana_program::rent::Rent;
use solana_program::system_instruction::{transfer, SystemInstruction};
use solana_program::{hash::Hash, native_token::LAMPORTS_PER_SOL, system_program};
use solana_program::instruction::AccountMeta;
use solana_program_test::{processor, BanksClient, ProgramTest};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use dlp::ephemeral_balance_seeds_from_payer;

mod fixtures;

const PRIZE: u64 = LAMPORTS_PER_SOL / 1000;

async fn setup_delegated_pda(program_test: &mut ProgramTest, authority_pubkey: &Pubkey) {
    // Setup a delegated PDA
    program_test.add_account(
        DELEGATED_PDA_ID,
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the delegation record PDA
    let delegation_record_data = get_delegation_record_data(*authority_pubkey, None);
    program_test.add_account(
        delegation_record_pda_from_delegated_account(&DELEGATED_PDA_ID),
        Account {
            lamports: Rent::default().minimum_balance(delegation_record_data.len()),
            data: delegation_record_data.clone(),
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the delegated account metadata PDA
    let delegation_metadata_data = get_delegation_metadata_data(*authority_pubkey, None);
    program_test.add_account(
        delegation_metadata_pda_from_delegated_account(&DELEGATED_PDA_ID),
        Account {
            lamports: Rent::default().minimum_balance(delegation_metadata_data.len()),
            data: delegation_metadata_data,
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
}

async fn setup_commit_state(program_test: &mut ProgramTest, authority_pubkey: &Pubkey) {
    // Setup the commit state PDA
    program_test.add_account(
        commit_state_pda_from_delegated_account(&DELEGATED_PDA_ID),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: COMMIT_NEW_STATE_ACCOUNT_DATA.into(),
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let commit_record_data = get_commit_record_account_data(*authority_pubkey);
    program_test.add_account(
        commit_record_pda_from_delegated_account(&DELEGATED_PDA_ID),
        Account {
            lamports: Rent::default().minimum_balance(commit_record_data.len()),
            data: commit_record_data,
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
}

async fn setup_escrow_account(program_test: &mut ProgramTest, authority_pubkey: &Pubkey) {
    let ephemeral_balance_pda = ephemeral_balance_pda_from_payer(&DELEGATED_PDA_ID, 0);

    // Setup the delegated account PDA
    program_test.add_account(
        ephemeral_balance_pda,
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the delegated record PDA
    let delegation_record_data =
        create_delegation_record_data(*authority_pubkey, dlp::id(), Some(LAMPORTS_PER_SOL));
    program_test.add_account(
        delegation_record_pda_from_delegated_account(&ephemeral_balance_pda),
        Account {
            lamports: Rent::default().minimum_balance(delegation_record_data.len()),
            data: delegation_record_data,
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the delegated account metadata PDA
    let delegation_metadata_data = create_delegation_metadata_data(
        *authority_pubkey,
        ephemeral_balance_seeds_from_payer!(DELEGATED_PDA_ID, 0),
        true,
    );
    program_test.add_account(
        delegation_metadata_pda_from_delegated_account(&ephemeral_balance_pda),
        Account {
            lamports: Rent::default().minimum_balance(delegation_metadata_data.len()),
            data: delegation_metadata_data,
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
}

async fn setup_program_test_env() -> (BanksClient, Keypair, Keypair, Hash) {
    let mut program_test = ProgramTest::new("dlp", dlp::ID, processor!(dlp::process_instruction));
    program_test.prefer_bpf(true);

    let authority = Keypair::from_bytes(&TEST_AUTHORITY).unwrap();

    // Setup authority
    program_test.add_account(
        authority.pubkey(),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup necessary accounts
    setup_delegated_pda(&mut program_test, &authority.pubkey()).await;
    setup_commit_state(&mut program_test, &authority.pubkey()).await;
    setup_escrow_account(&mut program_test, &authority.pubkey()).await;

    // Setup the validator fees vault
    program_test.add_account(
        validator_fees_vault_pda_from_validator(&authority.pubkey()),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks, payer, blockhash) = program_test.start().await;
    (banks, payer, authority, blockhash)
}

async fn create_escrow_account(banks: &BanksClient, payer: &Keypair, pubkey: &Pubkey) {
    // Top-up Ix
    let ix = dlp::instruction_builder::top_up_ephemeral_balance(
        payer.pubkey(),
        *pubkey,
        Some(0),
        Some(0),
    );

    // Delegate ephemeral balance Ix
    let delegate_ix = dlp::instruction_builder::delegate_ephemeral_balance(
        payer.pubkey(),
        *pubkey,
        DelegateEphemeralBalanceArgs::default(),
    );

    let recent_blockhash = banks
        .get_latest_blockhash()
        .await
        .expect("recent blockhash");
    let tx = Transaction::new_signed_with_payer(
        &[ix, delegate_ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );

    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok());
}

async fn fake_prize_transfer(banks: &BanksClient, payer: &Keypair, escrow_account: &Pubkey) {
    let ix = transfer(&payer.pubkey(), escrow_account, PRIZE);
    let recent_blockhash = banks
        .get_latest_blockhash()
        .await
        .expect("recent blockhash");

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );
    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok());

    let escrow_account = banks
        .get_account(*escrow_account)
        .await
        .expect("escrow account")
        .expect("exist");
    assert_eq!(escrow_account.lamports, PRIZE);
}

// 1. create escow
// 2. transfer funds to escrow from delegated
// commit escrow, then delegated
// finalize escrow

// async fn delegated_account_won_prize(banks: &BanksClient, from: &Keypair, delegated_account: &Pubkey) {
//     let ix = transfer(&from.pubkey(), delegated_account, PRIZE);
//     let recent_blockhash = banks.get_latest_blockhash().await.unwrap();
//
//     let tx = Transaction::new_signed_with_payer(
//         &[ix],
//         Some(&from.pubkey()),
//         &[&from],
//         recent_blockhash
//     );
//
//     let res = banks.process_transaction(tx).await;
//     println!("{:?}", res);
//     assert!(res.is_ok());
// }

async fn transfer_from_delegate_to_escrow(bank: &BanksClient) {}

#[tokio::test]
async fn test_finalize_with_data() {
    // Setup
    let (banks, _, authority, blockhash) = setup_program_test_env().await;

    // Retrieve the accounts
    let escrow_pda = ephemeral_balance_pda_from_payer(&DELEGATED_PDA_ID, 0);

    // println!("111");
    // // create & delegate escrow to dlp
    // create_escrow_account(&banks, &authority, &DELEGATED_PDA_ID).await;
    // // pretend that delegated account transfer it his prize.
    // // Here we skip that escrow would have to be committed from ER
    // println!("222");
    // fake_prize_transfer(&banks, &authority, &escrow_pda).await;

    // Submit the finalize with handler tx
    let destination = Keypair::new();
    let ix = dlp::instruction_builder::finalize_with_handler(
        authority.pubkey(),
        DELEGATED_PDA_ID,
        vec![],
        // vec![AccountMeta::new(destination.pubkey(), false)],
        dlp::ID,
        FinalizeWithDataArgs {
            escrow_index: 0,
            data: vec![]
        },
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&authority.pubkey()),
        &[&authority],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    println!("{:?}", res);
    assert!(res.is_ok());
}
