use crate::fixtures::{
    get_delegation_metadata_data_on_curve, get_delegation_record_on_curve_data, ON_CURVE_KEYPAIR,
    TEST_AUTHORITY,
};
use borsh::BorshDeserialize;
use dlp::instruction::CommitAccountArgs;
use dlp::pda::{
    committed_state_pda_from_pubkey, committed_state_record_pda_from_pubkey,
    delegation_metadata_pda_from_pubkey, delegation_record_pda_from_pubkey,
    validator_fees_vault_pda_from_pubkey,
};
use dlp::state::{CommitRecord, DelegationMetadata};
use dlp::utils_account::AccountDeserialize;
use solana_program::pubkey::Pubkey;
use solana_program::rent::Rent;
use solana_program::{hash::Hash, native_token::LAMPORTS_PER_SOL, system_program};
use solana_program_test::{processor, BanksClient, ProgramTest};
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

mod fixtures;

#[tokio::test]
async fn test_commit_finalize_system_account_after_balance_decrease() {
    // Setup
    let delegated_account = Keypair::from_bytes(&ON_CURVE_KEYPAIR).unwrap();
    let (mut banks, _, authority, blockhash) = setup_program_test_env(SetupProgramTestEnvArgs {
        delegated_account_init_lamports: LAMPORTS_PER_SOL,
        delegated_account_current_lamports: LAMPORTS_PER_SOL,
        validator_vault_init_lamports: Rent::default().minimum_balance(0),
        delegated_account: delegated_account.pubkey(),
    })
    .await;

    let new_delegated_account_lamports = LAMPORTS_PER_SOL - 100;

    commit_new_state(CommitNewStateArgs {
        banks: &mut banks,
        authority: &authority,
        blockhash,
        new_delegated_account_lamports,
        delegate_account: delegated_account.pubkey(),
    })
    .await;

    finalize_new_state(FinalizeNewStateArgs {
        banks: &mut banks,
        authority: &authority,
        blockhash,
        delegate_account: delegated_account.pubkey(),
    })
    .await;

    // Assert finalized lamports balance is correct
    let delegated_account = banks
        .get_account(delegated_account.pubkey())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(delegated_account.lamports, new_delegated_account_lamports);

    // Assert the vault own the difference
    let validator_vault = banks
        .get_account(validator_fees_vault_pda_from_pubkey(&authority.pubkey()))
        .await
        .unwrap()
        .unwrap();
    assert!(validator_vault.lamports >= Rent::default().minimum_balance(0) + 100);
}

#[tokio::test]
async fn test_commit_finalize_system_account_after_balance_increase() {
    // Setup
    let delegated_account = Keypair::from_bytes(&ON_CURVE_KEYPAIR).unwrap();
    let (mut banks, _, authority, blockhash) = setup_program_test_env(SetupProgramTestEnvArgs {
        delegated_account_init_lamports: LAMPORTS_PER_SOL,
        delegated_account_current_lamports: LAMPORTS_PER_SOL,
        validator_vault_init_lamports: Rent::default().minimum_balance(0),
        delegated_account: delegated_account.pubkey(),
    })
    .await;

    let new_delegated_account_lamports = LAMPORTS_PER_SOL + 100;

    commit_new_state(CommitNewStateArgs {
        banks: &mut banks,
        authority: &authority,
        blockhash,
        new_delegated_account_lamports,
        delegate_account: delegated_account.pubkey(),
    })
    .await;

    finalize_new_state(FinalizeNewStateArgs {
        banks: &mut banks,
        authority: &authority,
        blockhash,
        delegate_account: delegated_account.pubkey(),
    })
    .await;

    // Assert finalized lamports balance is correct
    let delegated_account = banks
        .get_account(delegated_account.pubkey())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(delegated_account.lamports, new_delegated_account_lamports);

    // Assert the vault own the difference
    let validator_vault = banks
        .get_account(validator_fees_vault_pda_from_pubkey(&authority.pubkey()))
        .await
        .unwrap()
        .unwrap();
    assert!(validator_vault.lamports >= Rent::default().minimum_balance(0));
}

#[tokio::test]
async fn test_commit_finalize_system_account_after_balance_decrease_and_increase_mainchain() {
    // Setup
    let delegated_account = Keypair::from_bytes(&ON_CURVE_KEYPAIR).unwrap();
    let (mut banks, _, authority, blockhash) = setup_program_test_env(SetupProgramTestEnvArgs {
        delegated_account_init_lamports: LAMPORTS_PER_SOL,
        delegated_account_current_lamports: LAMPORTS_PER_SOL + 9000, // Simulate someone transferring lamports to the delegated account
        validator_vault_init_lamports: Rent::default().minimum_balance(0),
        delegated_account: delegated_account.pubkey(),
    })
    .await;

    let new_delegated_account_lamports = LAMPORTS_PER_SOL - 100;

    commit_new_state(CommitNewStateArgs {
        banks: &mut banks,
        authority: &authority,
        blockhash,
        new_delegated_account_lamports,
        delegate_account: delegated_account.pubkey(),
    })
    .await;

    finalize_new_state(FinalizeNewStateArgs {
        banks: &mut banks,
        authority: &authority,
        blockhash,
        delegate_account: delegated_account.pubkey(),
    })
    .await;

    // Assert finalized lamports balance is correct
    let delegated_account = banks
        .get_account(delegated_account.pubkey())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        delegated_account.lamports,
        new_delegated_account_lamports + 9000
    );

    // Assert the vault own the difference
    let validator_vault = banks
        .get_account(validator_fees_vault_pda_from_pubkey(&authority.pubkey()))
        .await
        .unwrap()
        .unwrap();
    assert!(validator_vault.lamports >= Rent::default().minimum_balance(0));
}

struct FinalizeNewStateArgs<'a> {
    banks: &'a mut BanksClient,
    authority: &'a Keypair,
    blockhash: Hash,
    delegate_account: Pubkey,
}

async fn finalize_new_state(args: FinalizeNewStateArgs<'_>) {
    let ix = dlp::instruction::finalize(
        args.authority.pubkey(),
        args.delegate_account,
        args.authority.pubkey(),
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&args.authority.pubkey()),
        &[&args.authority],
        args.blockhash,
    );
    let res = args.banks.process_transaction(tx).await;
    assert!(res.is_ok());

    // Assert that the account owner is still the delegation program
    let pda_account = args
        .banks
        .get_account(args.delegate_account)
        .await
        .unwrap()
        .unwrap();
    assert!(pda_account.owner.eq(&dlp::id()));
}

struct CommitNewStateArgs<'a> {
    banks: &'a mut BanksClient,
    authority: &'a Keypair,
    blockhash: Hash,
    new_delegated_account_lamports: u64,
    delegate_account: Pubkey,
}

async fn commit_new_state(args: CommitNewStateArgs<'_>) {
    let commit_args = CommitAccountArgs {
        data: vec![],
        slot: 100,
        allow_undelegation: true,
        lamports: args.new_delegated_account_lamports,
    };

    // Commit the state for the delegated account
    let ix =
        dlp::instruction::commit_state(args.authority.pubkey(), args.delegate_account, commit_args);
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&args.authority.pubkey()),
        &[&args.authority],
        args.blockhash,
    );
    let res = args.banks.process_transaction(tx).await;
    println!("{:?}", res);
    assert!(res.is_ok());

    // Assert the state commitment was created and contains the new state
    let committed_state_pda = committed_state_pda_from_pubkey(&args.delegate_account);
    let new_state_account = args
        .banks
        .get_account(committed_state_pda)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(new_state_account.data, vec![] as Vec<u8>);

    // Check that the commit has enough collateral to finalize the proposed state diff
    let delegated_account = args
        .banks
        .get_account(args.delegate_account)
        .await
        .unwrap()
        .unwrap();
    assert!(
        args.new_delegated_account_lamports
            < new_state_account.lamports + delegated_account.lamports
    );

    // Assert the record about the commitment exists
    let state_commit_record_pda = committed_state_record_pda_from_pubkey(&args.delegate_account);
    let state_commit_record_account = args
        .banks
        .get_account(state_commit_record_pda)
        .await
        .unwrap()
        .unwrap();
    let state_commit_record =
        CommitRecord::try_from_bytes(&state_commit_record_account.data).unwrap();
    assert_eq!(state_commit_record.account, args.delegate_account);
    assert_eq!(state_commit_record.identity, args.authority.pubkey());
    assert_eq!(state_commit_record.slot, 100);

    let delegation_metadata_pda = delegation_metadata_pda_from_pubkey(&args.delegate_account);
    let delegation_metadata_account = args
        .banks
        .get_account(delegation_metadata_pda)
        .await
        .unwrap()
        .unwrap();
    let delegation_metadata =
        DelegationMetadata::try_from_slice(&delegation_metadata_account.data).unwrap();
    assert_eq!(delegation_metadata.is_undelegatable, true);
}

#[derive(Debug)]
struct SetupProgramTestEnvArgs {
    delegated_account_init_lamports: u64,
    delegated_account_current_lamports: u64,
    validator_vault_init_lamports: u64,
    delegated_account: Pubkey,
}

async fn setup_program_test_env(
    args: SetupProgramTestEnvArgs,
) -> (BanksClient, Keypair, Keypair, Hash) {
    let mut program_test = ProgramTest::new("dlp", dlp::ID, processor!(dlp::process_instruction));
    program_test.prefer_bpf(true);

    let validator_keypair = Keypair::from_bytes(&TEST_AUTHORITY).unwrap();

    program_test.add_account(
        validator_keypair.pubkey(),
        Account {
            lamports: 10 * LAMPORTS_PER_SOL,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup a delegated PDA
    program_test.add_account(
        args.delegated_account,
        Account {
            lamports: args.delegated_account_current_lamports,
            data: vec![],
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the delegated account metadata PDA
    let data =
        get_delegation_metadata_data_on_curve(Some(args.delegated_account_init_lamports), None);
    program_test.add_account(
        delegation_metadata_pda_from_pubkey(&args.delegated_account),
        Account {
            lamports: Rent::default().minimum_balance(data.len()),
            data,
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the delegated record PDA
    let data = get_delegation_record_on_curve_data(validator_keypair.pubkey());
    program_test.add_account(
        delegation_record_pda_from_pubkey(&args.delegated_account),
        Account {
            lamports: Rent::default().minimum_balance(data.len()),
            data,
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the validator fees vault
    program_test.add_account(
        validator_fees_vault_pda_from_pubkey(&validator_keypair.pubkey()),
        Account {
            lamports: args.validator_vault_init_lamports,
            data: vec![],
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks, payer, blockhash) = program_test.start().await;
    (banks, payer, validator_keypair, blockhash)
}
