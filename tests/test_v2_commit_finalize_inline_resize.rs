use std::ptr::read_unaligned;

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
    signature::{Keypair, Signer},
    transaction::Transaction,
};

use crate::fixtures::{
    DEFAULT_COMMIT_FREQUENCY_MS, DEFAULT_DELEGATION_SLOT, DELEGATED_PDA_ID,
    DELEGATED_PDA_OWNER_ID, TEST_AUTHORITY,
};

mod fixtures;

#[tokio::test]
async fn test_v2_commit_finalize_inline_resize_perf() {
    // Setup
    const KB: usize = 1024;
    const ACCOUNT_SIZE: usize = 4 * KB;

    let (banks, _, authority, blockhash) =
        setup_program_test_env(vec![0; ACCOUNT_SIZE]).await;
    let new_state: Vec<u8> = vec![1; ACCOUNT_SIZE + 0 * KB];

    let new_account_balance = 1_000_000;

    let ix = dlp::instruction_builder::v2_commit_finalize_inline_resize(
        authority.pubkey(),
        DELEGATED_PDA_ID,
        &mut CommitFinalizeArgs {
            commit_id: 1,
            allow_undelegation: true.into(),
            data_is_diff: false.into(),
            lamports: new_account_balance,
            reserved_padding: Default::default(),
        },
        &new_state,
    );
    let tx = Transaction::new_signed_with_payer(
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(
                dlp::instruction_builder::MAX_CU_COMMIT_FINALIZE + 150, /* 150 for ComputeBudgetInstruction itself */
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
    let state_size = unsafe {
        read_unaligned(delegated_account.data.as_ptr() as *const u32)
    } as usize;
    assert_eq!(
        delegated_account.data[4 + state_size..].len(),
        new_state.len()
    );
    assert_eq!(delegated_account.data[4 + state_size..], new_state);

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

    let validator_fees_vault =
        validator_fees_vault_pda_from_validator(&validator_keypair.pubkey());
    let state_header = DelegationStateHeader {
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

    // Setup a delegated PDA containing the delegation_state at the end
    program_test.add_account(
        DELEGATED_PDA_ID,
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: {
                let state_buffer = vec![];
                let state = [state_header.to_bytes(), state_buffer].concat();
                let state_size = (state.len() as u32).to_le_bytes().to_vec();

                [
                    state_size, // size of the delegation state
                    state,      // delegation state (appended by Delegate ix)
                    pda_data,   // delegated pda
                ]
                .concat()
            },
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

    let (banks, payer, blockhash) = program_test.start().await;
    (banks, payer, validator_keypair, blockhash)
}
