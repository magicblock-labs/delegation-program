use delegation_program_interface::{
    pda::{
        delegation_metadata_pda_from_delegated_account,
        delegation_record_pda_from_delegated_account,
    },
    v2::{
        instruction_builder::{
            approve_commitment, propose_commitment, register_operator,
            register_verifier, update_verifier_registry, write_state_buffer,
        },
        pda::pending_commitment_pda,
        PendingCommitment, ProposeCommitmentArgs, RegisterOperatorArgs,
        RegisterVerifierArgs, UpdateVerifierRegistryArgs,
        VerifierRegistryAction, WriteStateBufferArgs,
    },
};
use solana_program::native_token::LAMPORTS_PER_SOL;
use solana_program_test::{
    BanksClient, ProgramTest, ProgramTestBanksClientExt,
};
use solana_sdk::{
    account::Account,
    hash::Hash,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use solana_sdk_ids::system_program;
use wheels::layout::Decodable;

mod fixtures;

use crate::fixtures::{
    create_delegation_metadata_data_with_commit_id,
    create_delegation_record_data,
    v2::{challenge_cluster_args, initialize_delegation_cluster_with_identity},
};

#[tokio::test]
async fn test_approve_commitment() {
    let mut env = setup_approve_commitment_env(1).await;
    let args = propose_commitment_args();

    propose(
        &mut env.banks,
        &env.payer,
        &env.operator,
        env.delegation_cluster,
        env.delegated_account,
        args.clone(),
    )
    .await
    .unwrap();
    approve(
        &mut env.banks,
        &env.payer,
        &env.verifiers[0],
        env.delegation_cluster,
        env.delegated_account,
        args.commit_id,
    )
    .await
    .unwrap();

    let pending_commitment_data = read_pending_commitment_data(
        &mut env.banks,
        env.delegated_account,
        args.commit_id,
    )
    .await;
    let pending_commitment =
        <PendingCommitment as Decodable>::decode(&pending_commitment_data)
            .unwrap();
    assert_eq!(pending_commitment.approval_count(), 1);
    assert_eq!(pending_commitment.selected_verifiers().len(), 1);

    let selected_verifier =
        pending_commitment.selected_verifiers().get(0).unwrap();
    assert_eq!(
        *selected_verifier.verifier_identity(),
        env.verifiers[0].pubkey()
    );
    assert!(selected_verifier.approved());
}

#[tokio::test]
async fn test_approve_commitment_duplicate_is_noop() {
    let mut env = setup_approve_commitment_env(1).await;
    let args = propose_commitment_args();

    propose(
        &mut env.banks,
        &env.payer,
        &env.operator,
        env.delegation_cluster,
        env.delegated_account,
        args.clone(),
    )
    .await
    .unwrap();
    approve(
        &mut env.banks,
        &env.payer,
        &env.verifiers[0],
        env.delegation_cluster,
        env.delegated_account,
        args.commit_id,
    )
    .await
    .unwrap();
    approve(
        &mut env.banks,
        &env.payer,
        &env.verifiers[0],
        env.delegation_cluster,
        env.delegated_account,
        args.commit_id,
    )
    .await
    .unwrap();

    let pending_commitment_data = read_pending_commitment_data(
        &mut env.banks,
        env.delegated_account,
        args.commit_id,
    )
    .await;
    let pending_commitment =
        <PendingCommitment as Decodable>::decode(&pending_commitment_data)
            .unwrap();
    assert_eq!(pending_commitment.approval_count(), 1);
    assert!(pending_commitment
        .selected_verifiers()
        .get(0)
        .unwrap()
        .approved());
}

#[tokio::test]
async fn test_approve_commitment_fails_with_wrong_verifier() {
    let mut env = setup_approve_commitment_env(2).await;
    let args = propose_commitment_args();

    propose(
        &mut env.banks,
        &env.payer,
        &env.operator,
        env.delegation_cluster,
        env.delegated_account,
        args.clone(),
    )
    .await
    .unwrap();

    assert!(approve(
        &mut env.banks,
        &env.payer,
        &env.verifiers[1],
        env.delegation_cluster,
        env.delegated_account,
        args.commit_id,
    )
    .await
    .is_err());
}

#[tokio::test]
async fn test_approve_commitment_fails_without_verifier_signature() {
    let mut env = setup_approve_commitment_env(1).await;
    let args = propose_commitment_args();

    propose(
        &mut env.banks,
        &env.payer,
        &env.operator,
        env.delegation_cluster,
        env.delegated_account,
        args.clone(),
    )
    .await
    .unwrap();

    let mut ix = approve_commitment(
        env.verifiers[0].pubkey(),
        env.delegation_cluster,
        env.delegated_account,
        args.commit_id,
    );
    ix.accounts[0].is_signer = false;

    let blockhash = env.banks.get_latest_blockhash().await.unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&env.payer.pubkey()),
        &[&env.payer],
        blockhash,
    );

    assert!(env.banks.process_transaction(tx).await.is_err());
}

#[tokio::test]
async fn test_approve_commitment_fails_with_instruction_data() {
    let mut env = setup_approve_commitment_env(1).await;
    let args = propose_commitment_args();

    propose(
        &mut env.banks,
        &env.payer,
        &env.operator,
        env.delegation_cluster,
        env.delegated_account,
        args.clone(),
    )
    .await
    .unwrap();

    let mut ix = approve_commitment(
        env.verifiers[0].pubkey(),
        env.delegation_cluster,
        env.delegated_account,
        args.commit_id,
    );
    ix.data.push(1);

    let blockhash = env.banks.get_latest_blockhash().await.unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&env.payer.pubkey()),
        &[&env.payer, &env.verifiers[0]],
        blockhash,
    );

    assert!(env.banks.process_transaction(tx).await.is_err());
}

struct ApproveCommitmentEnv {
    banks: BanksClient,
    payer: Keypair,
    operator: Keypair,
    verifiers: Vec<Keypair>,
    delegation_cluster: Pubkey,
    delegated_account: Pubkey,
}

async fn setup_approve_commitment_env(
    verifier_count: usize,
) -> ApproveCommitmentEnv {
    let mut program_test = ProgramTest::new(
        "delegation_program",
        delegation_program_interface::ID,
        None,
    );
    program_test.prefer_bpf(true);

    let authority = Keypair::new();
    let operator = Keypair::new();
    let verifiers = (0..verifier_count)
        .map(|_| Keypair::new())
        .collect::<Vec<_>>();
    let delegated_account = Pubkey::new_unique();
    let cluster_identity = Keypair::new();
    let cluster_args = challenge_cluster_args(operator.pubkey());
    let delegation_cluster =
        delegation_program_interface::v2::pda::delegation_cluster_pda(
            &cluster_identity.pubkey(),
        );

    add_lamport_account(&mut program_test, authority.pubkey());
    add_lamport_account(&mut program_test, operator.pubkey());
    for verifier in verifiers.iter() {
        add_lamport_account(&mut program_test, verifier.pubkey());
    }

    program_test.add_account(
        delegated_account,
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: state_data(),
            owner: delegation_program_interface::ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    program_test.add_account(
        delegation_record_pda_from_delegated_account(&delegated_account),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: create_delegation_record_data(
                delegation_cluster,
                propose_commitment_args().owner,
                Some(LAMPORTS_PER_SOL),
            ),
            owner: delegation_program_interface::ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    program_test.add_account(
        delegation_metadata_pda_from_delegated_account(&delegated_account),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: create_delegation_metadata_data_with_commit_id(
                authority.pubkey(),
                &[b"test-pda"],
                false,
                0,
            ),
            owner: delegation_program_interface::ID,
            executable: false,
            rent_epoch: 0,
        },
    );

    let (mut banks, payer, blockhash) = program_test.start().await;
    initialize_delegation_cluster_with_identity(
        &banks,
        &payer,
        &authority,
        &cluster_identity,
        blockhash,
        cluster_args.clone(),
    )
    .await;

    let challenge_config = cluster_args.challenge_config.unwrap();
    register_operator_for_cluster(
        &banks,
        &payer,
        &operator,
        &authority,
        delegation_cluster,
        challenge_config.min_operator_bond,
    )
    .await;
    register_and_add_verifier(
        &banks,
        &payer,
        &operator,
        &authority,
        delegation_cluster,
        challenge_config.min_verifier_bond,
    )
    .await;

    for verifier in verifiers.iter() {
        register_and_add_verifier(
            &banks,
            &payer,
            verifier,
            &authority,
            delegation_cluster,
            challenge_config.min_verifier_bond,
        )
        .await;
    }

    write_state(
        &mut banks,
        &payer,
        &operator,
        delegated_account,
        WriteStateBufferArgs {
            commit_id: propose_commitment_args().commit_id,
            total_len: state_data().len() as u32,
            offset: 0,
            chunk: state_data(),
        },
    )
    .await;

    ApproveCommitmentEnv {
        banks,
        payer,
        operator,
        verifiers,
        delegation_cluster,
        delegated_account,
    }
}

fn add_lamport_account(program_test: &mut ProgramTest, pubkey: Pubkey) {
    program_test.add_account(
        pubkey,
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
}

async fn register_operator_for_cluster(
    banks: &BanksClient,
    payer: &Keypair,
    operator: &Keypair,
    authority: &Keypair,
    delegation_cluster: Pubkey,
    stake_lamports: u64,
) {
    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let ix = register_operator(
        operator.pubkey(),
        authority.pubkey(),
        delegation_cluster,
        RegisterOperatorArgs { stake_lamports },
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer, operator, authority],
        blockhash,
    );

    banks.process_transaction(tx).await.unwrap();
}

async fn register_and_add_verifier(
    banks: &BanksClient,
    payer: &Keypair,
    verifier: &Keypair,
    authority: &Keypair,
    delegation_cluster: Pubkey,
    stake_lamports: u64,
) {
    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let ix = register_verifier(
        verifier.pubkey(),
        authority.pubkey(),
        delegation_cluster,
        RegisterVerifierArgs { stake_lamports },
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer, verifier, authority],
        blockhash,
    );
    banks.process_transaction(tx).await.unwrap();

    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let ix = update_verifier_registry(
        authority.pubkey(),
        delegation_cluster,
        verifier.pubkey(),
        UpdateVerifierRegistryArgs {
            action: VerifierRegistryAction::Add.value(),
            weight: 1,
        },
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer, authority],
        blockhash,
    );

    banks.process_transaction(tx).await.unwrap();
}

fn propose_commitment_args() -> ProposeCommitmentArgs {
    ProposeCommitmentArgs {
        commit_id: 1,
        lamports: 1_000,
        owner: Pubkey::new_from_array([7; 32]),
        da_pointer_hash: [9; 32],
        er_slot: Some(42),
    }
}

async fn write_state(
    banks: &mut BanksClient,
    payer: &Keypair,
    operator: &Keypair,
    account: Pubkey,
    args: WriteStateBufferArgs,
) {
    let latest_blockhash: Hash = banks.get_latest_blockhash().await.unwrap();
    let blockhash = banks
        .get_new_latest_blockhash(&latest_blockhash)
        .await
        .unwrap();
    let ix =
        write_state_buffer(payer.pubkey(), operator.pubkey(), account, args);
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer, operator],
        blockhash,
    );

    banks.process_transaction(tx).await.unwrap();
}

fn state_data() -> Vec<u8> {
    vec![1, 2, 3, 4]
}

async fn propose(
    banks: &mut BanksClient,
    payer: &Keypair,
    operator: &Keypair,
    delegation_cluster: Pubkey,
    delegated_account: Pubkey,
    args: ProposeCommitmentArgs,
) -> Result<(), solana_program_test::BanksClientError> {
    let latest_blockhash: Hash = banks.get_latest_blockhash().await.unwrap();
    let blockhash = banks
        .get_new_latest_blockhash(&latest_blockhash)
        .await
        .unwrap();
    let ix = propose_commitment(
        operator.pubkey(),
        delegation_cluster,
        delegated_account,
        args,
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer, operator],
        blockhash,
    );

    banks.process_transaction(tx).await
}

async fn approve(
    banks: &mut BanksClient,
    payer: &Keypair,
    verifier: &Keypair,
    delegation_cluster: Pubkey,
    delegated_account: Pubkey,
    commit_id: u64,
) -> Result<(), solana_program_test::BanksClientError> {
    let latest_blockhash: Hash = banks.get_latest_blockhash().await.unwrap();
    let blockhash = banks
        .get_new_latest_blockhash(&latest_blockhash)
        .await
        .unwrap();
    let ix = approve_commitment(
        verifier.pubkey(),
        delegation_cluster,
        delegated_account,
        commit_id,
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer, verifier],
        blockhash,
    );

    banks.process_transaction(tx).await
}

async fn read_pending_commitment_data(
    banks: &mut BanksClient,
    delegated_account: Pubkey,
    commit_id: u64,
) -> Vec<u8> {
    let pending_commitment_account = banks
        .get_account(pending_commitment_pda(&delegated_account, commit_id))
        .await
        .unwrap()
        .unwrap();

    pending_commitment_account.data
}
