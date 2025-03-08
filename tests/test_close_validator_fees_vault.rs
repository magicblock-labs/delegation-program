use crate::fixtures::TEST_AUTHORITY;
use dlp::pda::validator_fees_vault_pda_from_validator;
use solana_program::{hash::Hash, native_token::LAMPORTS_PER_SOL, system_program};
use solana_program_test::{processor, BanksClient, ProgramTest};
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

mod fixtures;

#[tokio::test]
async fn test_close_validator_fees_vault() {
    // Setup
    let (banks, admin, validator, blockhash) = setup_program_test_env().await;

    let validator_fees_vault_pda = validator_fees_vault_pda_from_validator(&validator.pubkey());

    // Submit the close vault tx
    let ix = dlp::instruction_builder::close_validator_fees_vault(
        admin.pubkey(),
        admin.pubkey(),
        validator.pubkey(),
    );
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&admin.pubkey()), &[&admin], blockhash);
    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok());

    // Assert the validator fees vault now has been closed
    let validator_fees_vault_account = banks.get_account(validator_fees_vault_pda).await.unwrap();
    assert!(validator_fees_vault_account.is_none());
}

async fn setup_program_test_env() -> (BanksClient, Keypair, Keypair, Hash) {
    let mut program_test = ProgramTest::new("dlp", dlp::ID, processor!(dlp::process_instruction));
    program_test.prefer_bpf(true);

    let admin_keypair = Keypair::from_bytes(&TEST_AUTHORITY).unwrap();
    let validator = Keypair::new();

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

    // Setup the validator fees vault
    program_test.add_account(
        validator_fees_vault_pda_from_validator(&validator.pubkey()),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks, _, blockhash) = program_test.start().await;
    (banks, admin_keypair, validator, blockhash)
}
