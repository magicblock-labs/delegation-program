use dlp_api::{
    pda::delegation_record_pda_from_delegated_account,
    v2::{
        instruction_builder::{
            post_commitment, register_operator, register_verifier,
            update_verifier_registry,
        },
        pda::{pending_commitment_pda, verifier_registry_pda},
        PendingCommitment, PostCommitmentArgs, RegisterOperatorArgs,
        RegisterVerifierArgs, VerifierRegistry,
        PENDING_COMMITMENT_STATUS_ACTIVE, VERIFIER_REGISTRY_ACTION_ADD,
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
    v2::{init_v2, valid_args},
};

#[tokio::test]
async fn test_v2_post_commitment() {
    let mut env = setup_post_commitment_env(2, true, false).await;
    let args = valid_post_commitment_args();

    post_v2_commitment(&mut env, args.clone()).await.unwrap();

    let pending_commitment_account = env
        .banks
        .get_account(pending_commitment_pda(
            &env.delegated_account,
            args.commit_id,
        ))
        .await
        .unwrap()
        .unwrap();
    let pending_commitment = <PendingCommitment as Decodable>::decode(
        &pending_commitment_account.data,
    )
    .unwrap();

    assert_eq!(
        pending_commitment.discriminator(),
        PendingCommitment::DISCRIMINATOR
    );
    assert_eq!(
        pending_commitment.status(),
        PENDING_COMMITMENT_STATUS_ACTIVE
    );
    assert_eq!(
        *pending_commitment.operator_identity(),
        env.operator.pubkey()
    );
    assert_eq!(*pending_commitment.account_pubkey(), env.delegated_account);
    assert_eq!(pending_commitment.commit_id(), args.commit_id);
    assert_eq!(pending_commitment.lamports(), args.lamports);
    assert_eq!(*pending_commitment.owner(), args.owner);
    assert_eq!(*pending_commitment.data_hash(), args.data_hash);
    assert_eq!(*pending_commitment.da_pointer_hash(), args.da_pointer_hash);
    assert_eq!(pending_commitment.approval_count(), 0);
    assert_eq!(
        pending_commitment.approval_threshold(),
        env.config_args.approval_threshold
    );
    assert_eq!(
        pending_commitment.challenge_window_end_slot(),
        pending_commitment.posted_slot()
            + env.config_args.challenge_window_slots
    );

    let selected: Vec<_> = pending_commitment
        .selected_verifiers()
        .iter()
        .map(|entry| (*entry.verifier_identity(), entry.approved()))
        .collect();
    assert_eq!(selected.len(), 2);
    assert_eq!(selected[0], (env.verifiers[0].pubkey(), false));
    assert_eq!(selected[1], (env.verifiers[1].pubkey(), false));
    assert!(selected
        .iter()
        .all(|(verifier, _)| *verifier != env.operator.pubkey()));

    let verifier_registry_account = env
        .banks
        .get_account(verifier_registry_pda())
        .await
        .unwrap()
        .unwrap();
    let verifier_registry = <VerifierRegistry as Decodable>::decode(
        &verifier_registry_account.data,
    )
    .unwrap();

    assert_eq!(verifier_registry.next_selection_index(), 3);
}

#[tokio::test]
async fn test_v2_post_commitment_fails_twice() {
    let mut env = setup_post_commitment_env(2, true, false).await;
    let args = valid_post_commitment_args();

    post_v2_commitment(&mut env, args.clone()).await.unwrap();

    assert!(post_v2_commitment(&mut env, args).await.is_err());
}

#[tokio::test]
async fn test_v2_post_commitment_fails_without_operator_signature() {
    let env = setup_post_commitment_env(2, false, false).await;
    let args = valid_post_commitment_args();
    let mut ix =
        post_commitment(env.operator.pubkey(), env.delegated_account, args);
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
async fn test_v2_post_commitment_fails_without_enough_verifiers() {
    let mut env = setup_post_commitment_env(0, true, false).await;

    assert!(post_v2_commitment(&mut env, valid_post_commitment_args())
        .await
        .is_err());
}

#[tokio::test]
async fn test_v2_post_commitment_fails_with_wrong_delegation_authority() {
    let mut env = setup_post_commitment_env(2, true, true).await;

    assert!(post_v2_commitment(&mut env, valid_post_commitment_args())
        .await
        .is_err());
}

struct PostCommitmentEnv {
    banks: BanksClient,
    payer: Keypair,
    operator: Keypair,
    verifiers: Vec<Keypair>,
    delegated_account: Pubkey,
    config_args: dlp_api::v2::InitProtocolConfigArgs,
}

async fn setup_post_commitment_env(
    verifier_count: usize,
    register_operator_as_verifier: bool,
    wrong_delegation_authority: bool,
) -> PostCommitmentEnv {
    let mut program_test = ProgramTest::new("dlp", dlp_api::ID, None);
    program_test.prefer_bpf(true);

    let authority = Keypair::new();
    let operator = Keypair::new();
    let verifiers = (0..verifier_count)
        .map(|_| Keypair::new())
        .collect::<Vec<_>>();
    let delegated_account = Pubkey::new_unique();
    let delegation_authority = if wrong_delegation_authority {
        Pubkey::new_unique()
    } else {
        operator.pubkey()
    };

    add_lamport_account(&mut program_test, authority.pubkey());
    add_lamport_account(&mut program_test, operator.pubkey());
    for verifier in verifiers.iter() {
        add_lamport_account(&mut program_test, verifier.pubkey());
    }

    program_test.add_account(
        delegated_account,
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![1, 2, 3, 4],
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
                delegation_authority,
                Pubkey::new_unique(),
                Some(LAMPORTS_PER_SOL),
            ),
            owner: dlp_api::ID,
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks, payer, blockhash) = program_test.start().await;
    let config_args = valid_args();
    init_v2(&banks, &payer, &authority, blockhash, config_args.clone()).await;

    register_v2_operator(
        &banks,
        &payer,
        &operator,
        &authority,
        config_args.min_operator_bond,
    )
    .await;

    if register_operator_as_verifier {
        register_and_add_v2_verifier(
            &banks,
            &payer,
            &operator,
            &authority,
            config_args.min_verifier_bond,
        )
        .await;
    }

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

    PostCommitmentEnv {
        banks,
        payer,
        operator,
        verifiers,
        delegated_account,
        config_args,
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
        data_hash: [7; 32],
        da_pointer_hash: [9; 32],
        er_slot: Some(42),
    }
}

async fn post_v2_commitment(
    env: &mut PostCommitmentEnv,
    args: PostCommitmentArgs,
) -> Result<(), solana_program_test::BanksClientError> {
    let latest_blockhash: Hash =
        env.banks.get_latest_blockhash().await.unwrap();
    let blockhash = env
        .banks
        .get_new_latest_blockhash(&latest_blockhash)
        .await
        .unwrap();
    let ix =
        post_commitment(env.operator.pubkey(), env.delegated_account, args);
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&env.payer.pubkey()),
        &[&env.payer, &env.operator],
        blockhash,
    );

    env.banks.process_transaction(tx).await
}
