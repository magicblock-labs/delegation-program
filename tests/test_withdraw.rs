use crate::fixtures::EPHEMERAL_BALANCE_PDA;
use dlp::consts::FEES_VAULT;
use dlp::utils_account::AccountDeserialize;
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
async fn test_withdraw() {
    // Setup
    let (mut banks, _, payer_alt, blockhash) = setup_program_test_env().await;

    let fees_vault = Pubkey::find_program_address(&[FEES_VAULT], &dlp::id()).0;

    let init_lamports = banks
        .get_account(fees_vault)
        .await
        .unwrap()
        .unwrap()
        .lamports;

    let init_ephemeral_balance = banks
        .get_account(dlp::pda::ephemeral_balance_pda_from_pubkey(
            &payer_alt.pubkey(),
        ))
        .await
        .unwrap()
        .unwrap();
    let init_ephemeral_balance_data =
        dlp::state::EphemeralBalance::try_from_bytes(&init_ephemeral_balance.data).unwrap();
    let init_ephemeral_balance_lamports = init_ephemeral_balance_data.lamports;

    // Submit the undelegate tx
    let withdrawal_amount = 100000;
    let ix =
        dlp::instruction::withdraw_ephemeral_balance(payer_alt.pubkey(), Some(withdrawal_amount));
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer_alt.pubkey()),
        &[&payer_alt],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok());

    // Assert the fees vault now has less lamports
    let fees_vault_account = banks.get_account(fees_vault).await.unwrap();
    assert!(fees_vault_account.is_some());
    assert_eq!(
        fees_vault_account.unwrap().lamports,
        init_lamports - withdrawal_amount
    );

    // Assert the ephemeral balance account now has less lamports
    let ephemeral_balance_account = banks
        .get_account(dlp::pda::ephemeral_balance_pda_from_pubkey(
            &payer_alt.pubkey(),
        ))
        .await
        .unwrap();
    assert!(ephemeral_balance_account.is_some());
    let ephemeral_balance_account = ephemeral_balance_account.unwrap();
    let ephemeral_balance_data =
        dlp::state::EphemeralBalance::try_from_bytes(&ephemeral_balance_account.data).unwrap();
    assert_eq!(
        ephemeral_balance_data.lamports,
        init_ephemeral_balance_lamports - withdrawal_amount
    );
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

    // Setup the ephemeral balance account
    program_test.add_account(
        dlp::pda::ephemeral_balance_pda_from_pubkey(&payer_alt.pubkey()),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: EPHEMERAL_BALANCE_PDA.to_vec(),
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks, payer, blockhash) = program_test.start().await;
    (banks, payer, payer_alt, blockhash)
}
