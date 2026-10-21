use delegation_program_interface as dpi;
use dpi::v2::{
    instruction_builder::init_delegation_cluster, pda::delegation_cluster_pda,
    ChallengeConfig, InitDelegationClusterArgs,
};
use solana_program::{
    hash::Hash, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey,
};
use solana_program_test::{BanksClient, ProgramTest};
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use solana_sdk_ids::system_program;

pub fn direct_cluster_args(
    current_validator: Pubkey,
) -> InitDelegationClusterArgs {
    InitDelegationClusterArgs {
        current_validator,
        validator_rotation_authority: Pubkey::new_unique(),
        challenge_config: None,
    }
}

pub fn challenge_cluster_args(
    current_validator: Pubkey,
) -> InitDelegationClusterArgs {
    InitDelegationClusterArgs {
        challenge_config: Some(ChallengeConfig {
            resolver: Pubkey::new_unique(),
            min_operator_bond: 1,
            min_verifier_bond: 1,
            min_challenger_stake: 1,
            challenge_window_slots: 10,
            operator_response_timeout_slots: 10,
            challenger_reveal_timeout_slots: 10,
            payout_timelock_slots: 10,
            verifiers_per_commitment: 1,
            approval_threshold: 1,
            max_window_extensions: 1,
            match_penalty_bps: 500,
        }),
        ..direct_cluster_args(current_validator)
    }
}

#[allow(dead_code)]
pub async fn initialize_delegation_cluster(
    banks: &BanksClient,
    payer: &Keypair,
    config_authority: &Keypair,
    blockhash: Hash,
    args: InitDelegationClusterArgs,
) -> Pubkey {
    let cluster_identity = Keypair::new();
    initialize_delegation_cluster_with_identity(
        banks,
        payer,
        config_authority,
        &cluster_identity,
        blockhash,
        args,
    )
    .await
}

#[allow(dead_code)]
pub async fn initialize_delegation_cluster_with_identity(
    banks: &BanksClient,
    payer: &Keypair,
    config_authority: &Keypair,
    cluster_identity: &Keypair,
    blockhash: Hash,
    args: InitDelegationClusterArgs,
) -> Pubkey {
    let delegation_cluster = delegation_cluster_pda(&cluster_identity.pubkey());
    let ix = init_delegation_cluster(
        config_authority.pubkey(),
        cluster_identity.pubkey(),
        args,
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer, config_authority, cluster_identity],
        blockhash,
    );

    banks.process_transaction(tx).await.unwrap();
    delegation_cluster
}

pub async fn setup_program_test_env() -> (BanksClient, Keypair, Keypair, Hash) {
    let mut program_test =
        ProgramTest::new("delegation_program", dpi::ID, None);
    program_test.prefer_bpf(true);

    let authority = Keypair::new();
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

    let (banks, payer, blockhash) = program_test.start().await;
    (banks, payer, authority, blockhash)
}
