use dlp::{
    pda::validator_fees_vault_pda_from_validator,
    pod_view::PodView,
    v2::{
        CommitFinalizeArgs, DelegationStateHeader, ValidatedDelegationBindings,
    },
};
use solana_program::{
    hash::Hash, native_token::LAMPORTS_PER_SOL, rent::Rent, system_program,
};
use solana_program_test::{
    BanksClient, BanksTransactionResultWithMetadata, ProgramTest,
};
use solana_sdk::{
    account::Account,
    compute_budget::ComputeBudgetInstruction,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

use crate::fixtures::{
    DEFAULT_COMMIT_FREQUENCY_MS, DEFAULT_DELEGATION_SLOT, DELEGATED_PDA_ID,
    DELEGATED_PDA_OWNER_ID, TEST_AUTHORITY,
};

mod fixtures;

#[tokio::test]
async fn test_v2_commit_finalize_from_buffer_perf() {
    // Setup
    const KB: usize = 1024;
    const ACCOUNT_SIZE: usize = 4 * KB;

    let new_state: Vec<u8> = vec![1; ACCOUNT_SIZE];
    let (banks, _, authority, blockhash) =
        setup_program_test_env(vec![0; ACCOUNT_SIZE], new_state.clone()).await;

    let new_account_balance = 1_000_000;
    let state_buffer_pda =
        Pubkey::find_program_address(&[b"state_buffer"], &authority.pubkey()).0;

    let ix = dlp::instruction_builder::v2_commit_finalize_from_buffer(
        authority.pubkey(),
        DELEGATED_PDA_ID,
        state_buffer_pda,
        &mut CommitFinalizeArgs {
            commit_id: 1,
            allow_undelegation: true.into(),
            data_is_diff: false.into(),
            lamports: new_account_balance,
            reserved_padding: Default::default(),
        },
    );

    let tx = Transaction::new_signed_with_payer(
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(
                dlp::instruction_builder::MAX_CU_COMMIT_FINALIZE_FROM_BUFFER
                    + 150, /* 150 for ComputeBudgetInstruction itself */
            ),
            ix,
        ],
        Some(&authority.pubkey()),
        &[&authority],
        blockhash,
    );

    // execute CommitFinalize and validate CU performmace
    {
        let BanksTransactionResultWithMetadata {
            result: _,
            metadata,
        } = banks.process_transaction_with_metadata(tx).await.unwrap();

        let metadata = metadata.unwrap();

        assertables::assert_lt!(metadata.compute_units_consumed, 1150);

        assert_eq!(
            metadata
                .log_messages
                .iter()
                .filter(|log| !log
                    .contains("ComputeBudget111111111111111111111111111111"))
                .count(),
            3,
            "CommitFinalize must not log anything in OK scenario"
        );
    }

    let delegated_account =
        banks.get_account(DELEGATED_PDA_ID).await.unwrap().unwrap();
    assert_eq!(delegated_account.data, new_state);

    // let delegation_metadata_account = banks
    //     .get_account(pdas.delegation_metadata)
    //     .await
    //     .unwrap()
    //     .unwrap();

    // let delegation_metadata =
    //     DelegationMetadata::try_from_bytes_with_discriminator(
    //         &delegation_metadata_account.data,
    //     )
    //     .unwrap();

    // assert_eq!(delegation_metadata.is_undelegatable, true);
}

async fn setup_program_test_env(
    pda_data: Vec<u8>,
    pda_new_state: Vec<u8>,
) -> (BanksClient, Keypair, Keypair, Hash) {
    let mut program_test = ProgramTest::new("dlp", dlp::ID, None);
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
        DELEGATED_PDA_ID,
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: pda_data,
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let validator_fees_vault =
        validator_fees_vault_pda_from_validator(&validator_keypair.pubkey());
    let header = DelegationStateHeader {
        discriminator: DelegationStateHeader::DISCRIMINATOR,
        original_owner: DELEGATED_PDA_OWNER_ID.to_bytes().into(),
        delegation_slot: DEFAULT_DELEGATION_SLOT,
        original_lamports: Rent::default().minimum_balance(500),
        commit_frequency_ms: DEFAULT_COMMIT_FREQUENCY_MS,
        bindings: ValidatedDelegationBindings {
            delegated_account: DELEGATED_PDA_ID.to_bytes().into(),
            validator_as_authority: validator_keypair
                .pubkey()
                .to_bytes()
                .into(),
            validator_fees_vault: validator_fees_vault.to_bytes().into(),
        },
        last_commit_id: 0,
        rent_payer: validator_keypair.pubkey().to_bytes().into(),
        is_undelegatable: false.into(),
        reserved_padding0: Default::default(),
    };

    program_test.add_account(
        Pubkey::find_program_address(
            &[DelegationStateHeader::SEED, DELEGATED_PDA_ID.as_ref()],
            &dlp::id(),
        )
        .0,
        Account {
            lamports: Rent::default()
                .minimum_balance(DelegationStateHeader::SPACE),
            data: header.to_bytes(),
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the validator fees vault
    program_test.add_account(
        validator_fees_vault,
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    program_test.add_account(
        Pubkey::find_program_address(
            &[b"state_buffer"],
            &validator_keypair.pubkey(),
        )
        .0,
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: pda_new_state,
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks, payer, blockhash) = program_test.start().await;
    (banks, payer, validator_keypair, blockhash)
}
