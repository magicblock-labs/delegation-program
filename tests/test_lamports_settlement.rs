use crate::fixtures::{
    create_delegation_record_data, get_delegation_metadata_data,
    get_delegation_metadata_data_on_curve, COMMIT_NEW_STATE_ACCOUNT_DATA, DELEGATED_PDA_ID,
    DELEGATED_PDA_OWNER_ID, ON_CURVE_KEYPAIR, TEST_AUTHORITY,
};
use dlp::args::CommitStateArgs;
use dlp::pda::{
    commit_record_pda_from_delegated_account, commit_state_pda_from_delegated_account,
    delegation_metadata_pda_from_delegated_account, delegation_record_pda_from_delegated_account,
    fees_vault_pda, validator_fees_vault_pda_from_validator,
};
use dlp::state::{CommitRecord, DelegationMetadata};
use solana_program::pubkey::Pubkey;
use solana_program::rent::Rent;
use solana_program::{hash::Hash, native_token::LAMPORTS_PER_SOL, system_program};
use solana_program_test::{processor, read_file, BanksClient, ProgramTest};
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

mod fixtures;

#[tokio::test]
async fn test_commit_finalize_system_account_after_balance_decrease() {
    test_commit_system_account_after_balance_decrease(false, false).await;
}

#[tokio::test]
async fn test_commit_undelegate_system_account_after_balance_decrease() {
    test_commit_system_account_after_balance_decrease(true, false).await;
}

#[tokio::test]
async fn test_commit_finalize_pda_after_balance_decrease() {
    test_commit_system_account_after_balance_decrease(false, true).await;
}

#[tokio::test]
async fn test_commit_undelegate_pda_after_balance_decrease() {
    test_commit_system_account_after_balance_decrease(true, true).await;
}

#[tokio::test]
async fn test_commit_finalize_system_account_after_balance_increase() {
    test_commit_system_account_after_balance_increase(false, false).await;
}

#[tokio::test]
async fn test_commit_undelegate_system_account_after_balance_increase() {
    test_commit_system_account_after_balance_increase(true, false).await;
}

#[tokio::test]
async fn test_commit_finalize_pda_after_balance_increase() {
    test_commit_system_account_after_balance_increase(false, true).await;
}

#[tokio::test]
async fn test_commit_undelegate_pda_after_balance_increase() {
    test_commit_system_account_after_balance_increase(true, true).await;
}

#[tokio::test]
async fn test_commit_finalise_system_account_after_balance_decrease_and_increase_mainchain() {
    test_commit_system_account_after_balance_decrease_and_increase_mainchain(false, false).await;
}

#[tokio::test]
async fn test_commit_undelegate_system_account_after_balance_decrease_and_increase_mainchain() {
    test_commit_system_account_after_balance_decrease_and_increase_mainchain(true, false).await;
}

#[tokio::test]
async fn test_commit_finalise_pda_after_balance_decrease_and_increase_mainchain() {
    test_commit_system_account_after_balance_decrease_and_increase_mainchain(false, true).await;
}

#[tokio::test]
async fn test_commit_undelegate_pda_after_balance_decrease_and_increase_mainchain() {
    test_commit_system_account_after_balance_decrease_and_increase_mainchain(true, true).await;
}

#[tokio::test]
async fn test_commit_finalise_system_account_after_balance_increase_and_increase_mainchain() {
    test_commit_system_account_after_balance_increase_and_increase_mainchain(false, false).await;
}

#[tokio::test]
async fn test_commit_undelegate_system_account_after_balance_increase_and_increase_mainchain() {
    test_commit_system_account_after_balance_increase_and_increase_mainchain(true, false).await;
}

#[tokio::test]
async fn test_commit_finalise_pda_after_balance_increase_and_increase_mainchain() {
    test_commit_system_account_after_balance_increase_and_increase_mainchain(false, true).await;
}

#[tokio::test]
async fn test_commit_undelegate_pda_after_balance_increase_and_increase_mainchain() {
    test_commit_system_account_after_balance_increase_and_increase_mainchain(true, true).await;
}

pub async fn test_commit_system_account_after_balance_decrease(is_undelegate: bool, is_pda: bool) {
    // Setup
    let (delegated_account, owner_program) = get_delegated_account_and_owner(is_pda);
    let (mut banks, _, authority, blockhash) =
        setup_program_for_commit_test_env(SetupProgramCommitTestEnvArgs {
            delegated_account_init_lamports: LAMPORTS_PER_SOL,
            delegated_account_current_lamports: LAMPORTS_PER_SOL,
            validator_vault_init_lamports: Rent::default().minimum_balance(0),
            delegated_account,
            owner_program,
        })
        .await;

    let new_delegated_account_lamports = LAMPORTS_PER_SOL - 100;

    commit_new_state(CommitNewStateArgs {
        banks: &mut banks,
        authority: &authority,
        blockhash,
        new_delegated_account_lamports,
        delegate_account: delegated_account,
        delegate_account_owner: owner_program,
    })
    .await;

    finalize_and_maybe_undelegate(
        is_undelegate,
        delegated_account,
        &mut banks,
        &authority,
        blockhash,
        owner_program,
    )
    .await;

    // Assert finalized lamports balance is correct
    let delegated_account = banks.get_account(delegated_account).await.unwrap().unwrap();
    assert_eq!(delegated_account.lamports, new_delegated_account_lamports);

    // Assert the vault own the difference
    let validator_vault = banks
        .get_account(validator_fees_vault_pda_from_validator(&authority.pubkey()))
        .await
        .unwrap()
        .unwrap();
    assert!(validator_vault.lamports >= Rent::default().minimum_balance(0) + 100);
}

async fn test_commit_system_account_after_balance_increase(is_undelegate: bool, is_pda: bool) {
    // Setup
    let (delegated_account, owner_program) = get_delegated_account_and_owner(is_pda);
    let (mut banks, _, authority, blockhash) =
        setup_program_for_commit_test_env(SetupProgramCommitTestEnvArgs {
            delegated_account_init_lamports: LAMPORTS_PER_SOL,
            delegated_account_current_lamports: LAMPORTS_PER_SOL,
            validator_vault_init_lamports: Rent::default().minimum_balance(0),
            delegated_account,
            owner_program,
        })
        .await;

    let new_delegated_account_lamports = LAMPORTS_PER_SOL + 100;

    commit_new_state(CommitNewStateArgs {
        banks: &mut banks,
        authority: &authority,
        blockhash,
        new_delegated_account_lamports,
        delegate_account: delegated_account,
        delegate_account_owner: owner_program,
    })
    .await;

    finalize_and_maybe_undelegate(
        is_undelegate,
        delegated_account,
        &mut banks,
        &authority,
        blockhash,
        owner_program,
    )
    .await;

    // Assert finalized lamports balance is correct
    let delegated_account = banks.get_account(delegated_account).await.unwrap().unwrap();
    assert_eq!(delegated_account.lamports, new_delegated_account_lamports);

    // Assert the vault own the difference
    let validator_vault = banks
        .get_account(validator_fees_vault_pda_from_validator(&authority.pubkey()))
        .await
        .unwrap()
        .unwrap();
    assert!(validator_vault.lamports >= Rent::default().minimum_balance(0));
}

async fn test_commit_system_account_after_balance_decrease_and_increase_mainchain(
    is_undelegate: bool,
    is_pda: bool,
) {
    // Setup
    let (delegated_account, owner_program) = get_delegated_account_and_owner(is_pda);
    let (mut banks, _, authority, blockhash) =
        setup_program_for_commit_test_env(SetupProgramCommitTestEnvArgs {
            delegated_account_init_lamports: LAMPORTS_PER_SOL,
            delegated_account_current_lamports: LAMPORTS_PER_SOL + 9000, // Simulate someone transferring lamports to the delegated account
            validator_vault_init_lamports: Rent::default().minimum_balance(0),
            delegated_account,
            owner_program,
        })
        .await;

    let new_delegated_account_lamports = LAMPORTS_PER_SOL - 100;

    commit_new_state(CommitNewStateArgs {
        banks: &mut banks,
        authority: &authority,
        blockhash,
        new_delegated_account_lamports,
        delegate_account: delegated_account,
        delegate_account_owner: owner_program,
    })
    .await;

    finalize_and_maybe_undelegate(
        is_undelegate,
        delegated_account,
        &mut banks,
        &authority,
        blockhash,
        owner_program,
    )
    .await;

    // Assert finalized lamports balance is correct
    let delegated_account = banks.get_account(delegated_account).await.unwrap().unwrap();
    assert_eq!(
        delegated_account.lamports,
        new_delegated_account_lamports + 9000
    );

    // Assert the vault own the difference
    let validator_vault = banks
        .get_account(validator_fees_vault_pda_from_validator(&authority.pubkey()))
        .await
        .unwrap()
        .unwrap();
    assert!(validator_vault.lamports >= Rent::default().minimum_balance(0));
}

async fn test_commit_system_account_after_balance_increase_and_increase_mainchain(
    is_undelegate: bool,
    is_pda: bool,
) {
    // Setup
    let (delegated_account, owner_program) = get_delegated_account_and_owner(is_pda);
    let (mut banks, _, authority, blockhash) =
        setup_program_for_commit_test_env(SetupProgramCommitTestEnvArgs {
            delegated_account_init_lamports: LAMPORTS_PER_SOL,
            delegated_account_current_lamports: LAMPORTS_PER_SOL + 8200, // Simulate someone transferring lamports to the delegated account
            validator_vault_init_lamports: Rent::default().minimum_balance(0),
            delegated_account,
            owner_program,
        })
        .await;

    let new_delegated_account_lamports = LAMPORTS_PER_SOL + 300;

    commit_new_state(CommitNewStateArgs {
        banks: &mut banks,
        authority: &authority,
        blockhash,
        new_delegated_account_lamports,
        delegate_account: delegated_account,
        delegate_account_owner: owner_program,
    })
    .await;

    finalize_and_maybe_undelegate(
        is_undelegate,
        delegated_account,
        &mut banks,
        &authority,
        blockhash,
        owner_program,
    )
    .await;

    // Assert finalized lamports balance is correct
    let delegated_account = banks.get_account(delegated_account).await.unwrap().unwrap();
    assert_eq!(
        delegated_account.lamports,
        new_delegated_account_lamports + 8200
    );

    // Assert the vault own the difference
    let validator_vault = banks
        .get_account(validator_fees_vault_pda_from_validator(&authority.pubkey()))
        .await
        .unwrap()
        .unwrap();
    assert!(validator_vault.lamports >= Rent::default().minimum_balance(0));
}

fn get_delegated_account_and_owner(is_pda: bool) -> (Pubkey, Pubkey) {
    let (delegated_account, owner_program) = if is_pda {
        (DELEGATED_PDA_ID, DELEGATED_PDA_OWNER_ID)
    } else {
        (
            Keypair::from_bytes(&ON_CURVE_KEYPAIR).unwrap().pubkey(),
            system_program::id(),
        )
    };
    (delegated_account, owner_program)
}

async fn finalize_and_maybe_undelegate(
    is_undelegate: bool,
    delegated_account: Pubkey,
    banks: &mut BanksClient,
    authority: &Keypair,
    blockhash: Hash,
    owner_program: Pubkey,
) {
    finalize_new_state(FinalizeNewStateArgs {
        banks,
        authority,
        blockhash,
        delegate_account: delegated_account,
    })
    .await;
    if is_undelegate {
        undelegate(UndelegateArgs {
            banks,
            authority,
            blockhash,
            delegate_account: delegated_account,
            owner_program,
        })
        .await;
    }
}

struct UndelegateArgs<'a> {
    banks: &'a mut BanksClient,
    authority: &'a Keypair,
    blockhash: Hash,
    delegate_account: Pubkey,
    owner_program: Pubkey,
}

async fn undelegate(args: UndelegateArgs<'_>) {
    // Retrieve the accounts
    let delegation_record_pda =
        delegation_record_pda_from_delegated_account(&args.delegate_account);

    // Submit the undelegate tx
    let ix = dlp::instruction_builder::undelegate(
        args.authority.pubkey(),
        args.delegate_account,
        args.owner_program,
        args.authority.pubkey(),
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&args.authority.pubkey()),
        &[&args.authority],
        args.blockhash,
    );
    let res = args.banks.process_transaction(tx).await;
    println!("{:?}", res);
    assert!(res.is_ok());

    // Assert the delegation_record_pda was closed
    let delegation_record_account = args.banks.get_account(delegation_record_pda).await.unwrap();
    eprintln!("delegation_record_account:{:?}", delegation_record_account);
    assert!(delegation_record_account.is_none());

    // Assert the delegated metadata account pda was closed
    let delegation_metadata_pda =
        delegation_metadata_pda_from_delegated_account(&args.delegate_account);
    let delegation_metadata_account = args
        .banks
        .get_account(delegation_metadata_pda)
        .await
        .unwrap();
    assert!(delegation_metadata_account.is_none());

    // Assert that the account owner is now set to the original owner program
    let pda_account = args
        .banks
        .get_account(args.delegate_account)
        .await
        .unwrap()
        .unwrap();
    assert!(pda_account.owner.eq(&args.owner_program));
}

struct FinalizeNewStateArgs<'a> {
    banks: &'a mut BanksClient,
    authority: &'a Keypair,
    blockhash: Hash,
    delegate_account: Pubkey,
}

async fn finalize_new_state(args: FinalizeNewStateArgs<'_>) {
    let ix = dlp::instruction_builder::finalize(args.authority.pubkey(), args.delegate_account);
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
    delegate_account_owner: Pubkey,
}

async fn commit_new_state(args: CommitNewStateArgs<'_>) {
    let data = if args.delegate_account.eq(&DELEGATED_PDA_ID) {
        COMMIT_NEW_STATE_ACCOUNT_DATA.to_vec()
    } else {
        vec![]
    };
    let commit_args = CommitStateArgs {
        data: data.clone(),
        slot: 100,
        allow_undelegation: true,
        lamports: args.new_delegated_account_lamports,
    };

    // Commit the state for the delegated account
    let ix = dlp::instruction_builder::commit_state(
        args.authority.pubkey(),
        args.delegate_account,
        args.delegate_account_owner,
        commit_args,
    );
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
    let commit_state_pda = commit_state_pda_from_delegated_account(&args.delegate_account);
    let commit_state_account = args
        .banks
        .get_account(commit_state_pda)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(commit_state_account.data, data);

    // Check that the commit has enough collateral to finalize the proposed state diff
    let delegated_account = args
        .banks
        .get_account(args.delegate_account)
        .await
        .unwrap()
        .unwrap();
    assert!(
        args.new_delegated_account_lamports
            < commit_state_account.lamports + delegated_account.lamports
    );

    // Assert the record about the commitment exists
    let commit_record_pda = commit_record_pda_from_delegated_account(&args.delegate_account);
    let commit_record_account = args
        .banks
        .get_account(commit_record_pda)
        .await
        .unwrap()
        .unwrap();
    let commit_record =
        CommitRecord::try_from_bytes_with_discriminator(&commit_record_account.data).unwrap();
    assert_eq!(commit_record.account, args.delegate_account);
    assert_eq!(commit_record.identity, args.authority.pubkey());
    assert_eq!(commit_record.slot, 100);

    let delegation_metadata_pda =
        delegation_metadata_pda_from_delegated_account(&args.delegate_account);
    let delegation_metadata_account = args
        .banks
        .get_account(delegation_metadata_pda)
        .await
        .unwrap()
        .unwrap();
    let delegation_metadata =
        DelegationMetadata::try_from_bytes_with_discriminator(&delegation_metadata_account.data)
            .unwrap();
    assert!(delegation_metadata.is_undelegatable);
}

#[derive(Debug)]
struct SetupProgramCommitTestEnvArgs {
    delegated_account_init_lamports: u64,
    delegated_account_current_lamports: u64,
    validator_vault_init_lamports: u64,
    delegated_account: Pubkey,
    owner_program: Pubkey,
}

async fn setup_program_for_commit_test_env(
    args: SetupProgramCommitTestEnvArgs,
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
    let data = if args.owner_program.eq(&DELEGATED_PDA_OWNER_ID) {
        get_delegation_metadata_data(validator_keypair.pubkey(), None)
    } else {
        get_delegation_metadata_data_on_curve(validator_keypair.pubkey(), None)
    };
    program_test.add_account(
        delegation_metadata_pda_from_delegated_account(&args.delegated_account),
        Account {
            lamports: Rent::default().minimum_balance(data.len()),
            data,
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the delegated record PDA
    let delegation_record_data = create_delegation_record_data(
        validator_keypair.pubkey(),
        args.owner_program,
        Some(args.delegated_account_init_lamports),
    );
    program_test.add_account(
        delegation_record_pda_from_delegated_account(&args.delegated_account),
        Account {
            lamports: Rent::default().minimum_balance(delegation_record_data.len()),
            data: delegation_record_data,
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the protocol fees vault
    program_test.add_account(
        fees_vault_pda(),
        Account {
            lamports: Rent::default().minimum_balance(0),
            data: vec![],
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the validator fees vault
    program_test.add_account(
        validator_fees_vault_pda_from_validator(&validator_keypair.pubkey()),
        Account {
            lamports: args.validator_vault_init_lamports,
            data: vec![],
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let data = read_file("tests/buffers/test_delegation.so");
    program_test.add_account(
        DELEGATED_PDA_OWNER_ID,
        Account {
            lamports: Rent::default().minimum_balance(data.len()),
            data,
            owner: solana_sdk::bpf_loader::id(),
            executable: true,
            rent_epoch: 0,
        },
    );

    let (banks, payer, blockhash) = program_test.start().await;
    (banks, payer, validator_keypair, blockhash)
}
