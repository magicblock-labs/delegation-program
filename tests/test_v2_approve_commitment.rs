use dlp_api::{
    pda::delegation_record_pda_from_delegated_account,
    v2::{
        instruction_builder::{
            approve_commitment, post_commitment, register_operator,
            register_verifier, update_verifier_registry, write_state_buffer,
        },
        pda::pending_commitment_pda,
        PendingCommitment, PostCommitmentArgs, RegisterOperatorArgs,
        RegisterVerifierArgs, WriteStateBufferArgs,
        VERIFIER_REGISTRY_ACTION_ADD,
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
    create_delegation_record_data,
    v2::{initialize_protocol_config, valid_protocol_config_args},
};

#[tokio::test]
async fn test_approve_commitment() {
    let mut env = setup_approve_commitment_env(1).await;
    let args = valid_post_commitment_args();

    post_v2_commitment(
        &mut env.banks,
        &env.payer,
        &env.operator,
        env.delegated_account,
        args.clone(),
    )
    .await
    .unwrap();
    approve_v2_commitment(
        &mut env.banks,
        &env.payer,
        &env.verifiers[0],
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
    let args = valid_post_commitment_args();

    post_v2_commitment(
        &mut env.banks,
        &env.payer,
        &env.operator,
        env.delegated_account,
        args.clone(),
    )
    .await
    .unwrap();
    approve_v2_commitment(
        &mut env.banks,
        &env.payer,
        &env.verifiers[0],
        env.delegated_account,
        args.commit_id,
    )
    .await
    .unwrap();
    approve_v2_commitment(
        &mut env.banks,
        &env.payer,
        &env.verifiers[0],
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
    let args = valid_post_commitment_args();

    post_v2_commitment(
        &mut env.banks,
        &env.payer,
        &env.operator,
        env.delegated_account,
        args.clone(),
    )
    .await
    .unwrap();

    assert!(approve_v2_commitment(
        &mut env.banks,
        &env.payer,
        &env.verifiers[1],
        env.delegated_account,
        args.commit_id,
    )
    .await
    .is_err());
}

#[tokio::test]
async fn test_approve_commitment_fails_without_verifier_signature() {
    let mut env = setup_approve_commitment_env(1).await;
    let args = valid_post_commitment_args();

    post_v2_commitment(
        &mut env.banks,
        &env.payer,
        &env.operator,
        env.delegated_account,
        args.clone(),
    )
    .await
    .unwrap();

    let mut ix = approve_commitment(
        env.verifiers[0].pubkey(),
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
    let args = valid_post_commitment_args();

    post_v2_commitment(
        &mut env.banks,
        &env.payer,
        &env.operator,
        env.delegated_account,
        args.clone(),
    )
    .await
    .unwrap();

    let mut ix = approve_commitment(
        env.verifiers[0].pubkey(),
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
    delegated_account: Pubkey,
}

async fn setup_approve_commitment_env(
    verifier_count: usize,
) -> ApproveCommitmentEnv {
    let mut program_test = ProgramTest::new("dlp", dlp_api::ID, None);
    program_test.prefer_bpf(true);

    let authority = Keypair::new();
    let operator = Keypair::new();
    let verifiers = (0..verifier_count)
        .map(|_| Keypair::new())
        .collect::<Vec<_>>();
    let delegated_account = Pubkey::new_unique();

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
            owner: dlp_api::ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    program_test.add_account(
        delegation_record_pda_from_delegated_account(&delegated_account),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: create_delegation_record_data(
                operator.pubkey(),
                Pubkey::new_unique(),
                Some(LAMPORTS_PER_SOL),
            ),
            owner: dlp_api::ID,
            executable: false,
            rent_epoch: 0,
        },
    );

    let (mut banks, payer, blockhash) = program_test.start().await;
    let config_args = valid_protocol_config_args();
    initialize_protocol_config(
        &banks,
        &payer,
        &authority,
        blockhash,
        config_args.clone(),
    )
    .await;

    register_v2_operator(
        &banks,
        &payer,
        &operator,
        &authority,
        config_args.min_operator_bond,
    )
    .await;
    register_and_add_v2_verifier(
        &banks,
        &payer,
        &operator,
        &authority,
        config_args.min_verifier_bond,
    )
    .await;

    for verifier in verifiers.iter() {
        register_and_add_v2_verifier(
            &banks,
            &payer,
            verifier,
            &authority,
            config_args.min_verifier_bond,
        )
        .await;
    }

    write_v2_state_buffer(
        &mut banks,
        &payer,
        &operator,
        delegated_account,
        WriteStateBufferArgs {
            commit_id: valid_post_commitment_args().commit_id,
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

async fn register_v2_operator(
    banks: &BanksClient,
    payer: &Keypair,
    operator: &Keypair,
    authority: &Keypair,
    amount_lamports: u64,
) {
    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let ix = register_operator(
        operator.pubkey(),
        authority.pubkey(),
        RegisterOperatorArgs { amount_lamports },
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer, operator, authority],
        blockhash,
    );

    banks.process_transaction(tx).await.unwrap();
}

async fn register_and_add_v2_verifier(
    banks: &BanksClient,
    payer: &Keypair,
    verifier: &Keypair,
    authority: &Keypair,
    amount_lamports: u64,
) {
    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let ix = register_verifier(
        verifier.pubkey(),
        authority.pubkey(),
        RegisterVerifierArgs { amount_lamports },
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
        verifier.pubkey(),
        dlp_api::v2::UpdateVerifierRegistryArgs {
            action: VERIFIER_REGISTRY_ACTION_ADD,
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

fn valid_post_commitment_args() -> PostCommitmentArgs {
    PostCommitmentArgs {
        commit_id: 1,
        lamports: 1_000,
        owner: Pubkey::new_unique(),
        da_pointer_hash: [9; 32],
        er_slot: Some(42),
    }
}

async fn write_v2_state_buffer(
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

async fn post_v2_commitment(
    banks: &mut BanksClient,
    payer: &Keypair,
    operator: &Keypair,
    delegated_account: Pubkey,
    args: PostCommitmentArgs,
) -> Result<(), solana_program_test::BanksClientError> {
    let latest_blockhash: Hash = banks.get_latest_blockhash().await.unwrap();
    let blockhash = banks
        .get_new_latest_blockhash(&latest_blockhash)
        .await
        .unwrap();
    let ix = post_commitment(operator.pubkey(), delegated_account, args);
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer, operator],
        blockhash,
    );

    banks.process_transaction(tx).await
}

async fn approve_v2_commitment(
    banks: &mut BanksClient,
    payer: &Keypair,
    verifier: &Keypair,
    delegated_account: Pubkey,
    commit_id: u64,
) -> Result<(), solana_program_test::BanksClientError> {
    let latest_blockhash: Hash = banks.get_latest_blockhash().await.unwrap();
    let blockhash = banks
        .get_new_latest_blockhash(&latest_blockhash)
        .await
        .unwrap();
    let ix =
        approve_commitment(verifier.pubkey(), delegated_account, commit_id);
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
