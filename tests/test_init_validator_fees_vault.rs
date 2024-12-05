use crate::fixtures::TEST_AUTHORITY;
use dlp::pda::validator_fees_vault_pda_from_pubkey;
use solana_program::pubkey::Pubkey;
use solana_program::{hash::Hash, native_token::LAMPORTS_PER_SOL, system_program};
use solana_program_test::{processor, BanksClient, ProgramTest};
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

mod fixtures;

#[tokio::test]
async fn test_init_validator_fees_vault() {
    // Setup
    let (mut banks, payer, admin, blockhash) = setup_program_test_env().await;

    let validator_identity = Pubkey::new_unique();
    let ix = dlp::instruction_builder::init_validator_fees_vault(
        payer.pubkey(),
        admin.pubkey(),
        validator_identity,
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &admin],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok());

    // Assert the fees vault was created successfully
    let fees_vault = validator_fees_vault_pda_from_pubkey(&validator_identity);
    let fees_vault_account = banks.get_account(fees_vault).await.unwrap();
    assert!(fees_vault_account.is_some());

    // Assert record cannot be created if the admin is not the correct one
    let validator_identity = Pubkey::new_unique();
    let ix = dlp::instruction_builder::init_validator_fees_vault(
        payer.pubkey(),
        payer.pubkey(),
        validator_identity,
    );
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[&payer], blockhash);
    let res = banks.process_transaction(tx).await;
    assert!(res.is_err());
}

async fn setup_program_test_env() -> (BanksClient, Keypair, Keypair, Hash) {
    let mut program_test = ProgramTest::new("dlp", dlp::ID, processor!(dlp::process_instruction));
    program_test.prefer_bpf(true);

    let admin_keypair = Keypair::from_bytes(&TEST_AUTHORITY).unwrap();

    program_test.add_account(
        admin_keypair.pubkey(),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks, payer, blockhash) = program_test.start().await;
    (banks, payer, admin_keypair, blockhash)
}
