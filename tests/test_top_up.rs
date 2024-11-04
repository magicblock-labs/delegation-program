use dlp::consts::FEES_VAULT;
use dlp::utils::utils_account::AccountDeserialize;
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
async fn test_top_up() {
    // Setup
    let (mut banks, payer, _, blockhash) = setup_program_test_env().await;

    let fees_vault = Pubkey::find_program_address(&[FEES_VAULT], &dlp::id()).0;

    let init_lamports = banks
        .get_account(fees_vault)
        .await
        .unwrap()
        .unwrap()
        .lamports;

    // Submit the undelegate tx
    let ix = dlp::instruction::top_up_ephemeral_balance(payer.pubkey(), 100_000);
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[&payer], blockhash);
    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok());

    // Assert the fees vault was created
    let fees_vault_account = banks.get_account(fees_vault).await.unwrap();
    assert!(fees_vault_account.is_some());
    assert_eq!(
        fees_vault_account.unwrap().lamports,
        init_lamports + 100_000
    );

    // Assert the ephemeral balance was created
    let ephemeral_balance = dlp::pda::ephemeral_balance_pda_from_pubkey(&payer.pubkey());
    let ephemeral_balance_account = banks.get_account(ephemeral_balance).await.unwrap();
    assert!(ephemeral_balance_account.is_some());
    let ephemeral_balance_account = ephemeral_balance_account.unwrap();
    let ephemeral_balance_data =
        dlp::state::EphemeralBalance::try_from_bytes(&ephemeral_balance_account.data).unwrap();
    println!("{:?}", &ephemeral_balance_account.data);
    assert_eq!(ephemeral_balance_data.lamports, 100_000);
}

async fn setup_program_test_env() -> (BanksClient, Keypair, Keypair, Hash) {
    let mut program_test = ProgramTest::new("dlp", dlp::ID, processor!(dlp::process_instruction));
    program_test.prefer_bpf(true);
    let payer_alt = Keypair::new();

    program_test.add_account(
        payer_alt.pubkey(),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the fees vault account
    program_test.add_account(
        Pubkey::find_program_address(&[FEES_VAULT], &dlp::id()).0,
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks, payer, blockhash) = program_test.start().await;
    (banks, payer, payer_alt, blockhash)
}
