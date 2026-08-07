use dlp_api::v2::{
    instruction_builder::init_protocol_config, InitProtocolConfigArgs,
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

pub fn valid_args() -> InitProtocolConfigArgs {
    InitProtocolConfigArgs {
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
    }
}

#[allow(dead_code)]
pub async fn init_v2(
    banks: &BanksClient,
    payer: &Keypair,
    authority: &Keypair,
    blockhash: Hash,
    args: InitProtocolConfigArgs,
) {
    let ix = init_protocol_config(authority.pubkey(), args);
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer, authority],
        blockhash,
    );

    banks.process_transaction(tx).await.unwrap();
}

pub async fn setup_program_test_env() -> (BanksClient, Keypair, Keypair, Hash) {
    let mut program_test = ProgramTest::new("dlp", dlp_api::ID, None);
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
