use delegation_program::solana_program;
use delegation_program_interface as dpi;
use dpi::{
    consts::EXTERNAL_UNDELEGATE_DISCRIMINATOR,
    instruction_builder::undelegate,
    pda::{
        delegation_cluster_pda, delegation_state_pda, undelegate_buffer_pda,
    },
    DelegationCluster, DelegationSeed, DelegationState, UndelegationRequester,
};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    native_token::LAMPORTS_PER_SOL,
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{
    processor, BanksClient, BanksClientError, ProgramTest,
};
use solana_sdk::{
    account::Account,
    instruction::InstructionError,
    signature::{Keypair, Signer},
    transaction::{Transaction, TransactionError},
};
use solana_sdk_ids::system_program;
use wheels::layout::Encodable;

const OWNER_PROGRAM_ID: Pubkey =
    solana_sdk::pubkey!("8qbHbw2BbbTHBW1sbeqakYXV7xPC7rULpRvbi3L6jUjL");
const ACCOUNT_DATA: &[u8] = b"finalized account state";
const ACCOUNT_SEED: &[u8] = b"undelegate-account-test";
const ACCOUNT_LAMPORTS: u64 = LAMPORTS_PER_SOL;

struct UndelegateTestContext {
    banks: BanksClient,
    payer: Keypair,
    current_validator: Keypair,
    delegated_account: Pubkey,
    delegation_state: Pubkey,
    delegation_cluster: Pubkey,
    delegation_rent_payer: Pubkey,
    original_owner: Pubkey,
    state_rent: u64,
}

#[tokio::test]
async fn test_undelegate_assigns_account_without_recreating_it() {
    let context = setup_undelegate_test(
        ACCOUNT_DATA.to_vec(),
        OWNER_PROGRAM_ID,
        UndelegationRequester::Validator,
        true,
    )
    .await;
    let validator_lamports_before = context
        .banks
        .get_account(context.current_validator.pubkey())
        .await
        .unwrap()
        .unwrap()
        .lamports;
    let rent_payer_lamports_before = context
        .banks
        .get_account(context.delegation_rent_payer)
        .await
        .unwrap()
        .unwrap()
        .lamports;

    submit_undelegate(&context, &context.current_validator)
        .await
        .unwrap();

    let delegated_account = context
        .banks
        .get_account(context.delegated_account)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(delegated_account.owner, OWNER_PROGRAM_ID);
    assert_eq!(delegated_account.data, ACCOUNT_DATA);
    assert_eq!(delegated_account.lamports, ACCOUNT_LAMPORTS);
    assert!(context
        .banks
        .get_account(context.delegation_state)
        .await
        .unwrap()
        .is_none());
    assert!(context
        .banks
        .get_account(undelegate_buffer_pda(&context.delegated_account).0)
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        context
            .banks
            .get_account(context.current_validator.pubkey())
            .await
            .unwrap()
            .unwrap()
            .lamports,
        validator_lamports_before
    );
    assert_eq!(
        context
            .banks
            .get_account(context.delegation_rent_payer)
            .await
            .unwrap()
            .unwrap()
            .lamports,
        rent_payer_lamports_before + context.state_rent
    );
}

#[tokio::test]
async fn test_undelegate_empty_system_account_without_callback() {
    let context = setup_undelegate_test(
        vec![],
        system_program::id(),
        UndelegationRequester::Validator,
        false,
    )
    .await;

    submit_undelegate(&context, &context.current_validator)
        .await
        .unwrap();

    let delegated_account = context
        .banks
        .get_account(context.delegated_account)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(delegated_account.owner, system_program::id());
    assert!(delegated_account.data.is_empty());
    assert_eq!(delegated_account.lamports, ACCOUNT_LAMPORTS);
}

#[tokio::test]
async fn test_undelegate_rejects_unrequested_and_owner_requested_accounts() {
    for requester in [
        UndelegationRequester::None,
        UndelegationRequester::OwnerProgram,
    ] {
        let context = setup_undelegate_test(
            ACCOUNT_DATA.to_vec(),
            OWNER_PROGRAM_ID,
            requester,
            false,
        )
        .await;
        let error = submit_undelegate(&context, &context.current_validator)
            .await
            .unwrap_err();
        assert_custom_error(
            error,
            match requester {
                UndelegationRequester::None => {
                    dpi::error::DlpError::NotUndelegatable
                }
                UndelegationRequester::OwnerProgram => {
                    dpi::error::DlpError::OwnerRequestedUndelegation
                }
                UndelegationRequester::Validator => unreachable!(),
            },
        );
    }
}

#[tokio::test]
async fn test_undelegate_rejects_data_above_one_growth_step() {
    let context = setup_undelegate_test(
        vec![1; pinocchio::account::MAX_PERMITTED_DATA_INCREASE + 1],
        OWNER_PROGRAM_ID,
        UndelegationRequester::Validator,
        false,
    )
    .await;

    assert_custom_error(
        submit_undelegate(&context, &context.current_validator)
            .await
            .unwrap_err(),
        dpi::error::DlpError::AccountDataTooLargeForUndelegation,
    );
}

async fn submit_undelegate(
    context: &UndelegateTestContext,
    validator: &Keypair,
) -> Result<(), BanksClientError> {
    let instruction = undelegate(
        validator.pubkey(),
        context.delegated_account,
        context.original_owner,
        context.delegation_cluster,
        context.delegation_rent_payer,
    )
    .unwrap();
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&context.payer.pubkey()),
        &[&context.payer, validator],
        context.banks.get_latest_blockhash().await.unwrap(),
    );

    context.banks.process_transaction(transaction).await
}

async fn setup_undelegate_test(
    account_data: Vec<u8>,
    original_owner: Pubkey,
    requester: UndelegationRequester,
    paused: bool,
) -> UndelegateTestContext {
    let mut program_test = ProgramTest::default();
    program_test.prefer_bpf(true);
    program_test.add_program("delegation_program", dpi::ID, None);
    program_test.prefer_bpf(false);
    program_test.add_program(
        "undelegate-account-owner",
        OWNER_PROGRAM_ID,
        processor!(owner_program_processor),
    );

    let current_validator = Keypair::new();
    let delegation_rent_payer = Pubkey::new_unique();
    let (delegated_account, delegated_account_bump, encoded_seeds) =
        if original_owner == system_program::id() {
            (Pubkey::new_unique(), 0, vec![])
        } else {
            let (delegated_account, bump) =
                Pubkey::find_program_address(&[ACCOUNT_SEED], &original_owner);
            (
                delegated_account,
                bump,
                DelegationSeed {
                    value: ACCOUNT_SEED.to_vec(),
                }
                .encode()
                .unwrap(),
            )
        };
    let cluster_id = Pubkey::new_unique();
    let delegation_cluster = delegation_cluster_pda(&cluster_id);
    let (_, cluster_bump) = Pubkey::find_program_address(
        &[dpi::pda::DELEGATION_CLUSTER_SEED, cluster_id.as_ref()],
        &dpi::id(),
    );
    let delegation_state = delegation_state_pda(&delegated_account);
    let (_, state_bump) = Pubkey::find_program_address(
        &[dpi::pda::DELEGATION_STATE_SEED, delegated_account.as_ref()],
        &dpi::id(),
    );

    program_test.add_account(
        current_validator.pubkey(),
        Account {
            lamports: 10 * LAMPORTS_PER_SOL,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
    program_test.add_account(
        delegation_rent_payer,
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
    program_test.add_account(
        delegated_account,
        Account {
            lamports: ACCOUNT_LAMPORTS,
            data: account_data,
            owner: dpi::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let cluster_data = DelegationCluster {
        discriminator: DelegationCluster::DISCRIMINATOR,
        bump: cluster_bump,
        cluster_id,
        config_authority: Pubkey::new_unique(),
        validator_rotation_authority: Pubkey::new_unique(),
        current_validator: current_validator.pubkey(),
        paused,
        protocol_fee_vault: Pubkey::new_unique(),
        challenge_config: None,
    }
    .encode()
    .unwrap();
    program_test.add_account(
        delegation_cluster,
        Account {
            lamports: Rent::default().minimum_balance(cluster_data.len()),
            data: cluster_data,
            owner: dpi::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let state_data = DelegationState {
        discriminator: DelegationState::DISCRIMINATOR,
        bump: state_bump,
        delegated_account,
        delegation_cluster,
        original_owner,
        rent_payer: delegation_rent_payer,
        delegation_slot: 0,
        lamports: ACCOUNT_LAMPORTS,
        last_commit_seq: 1,
        undelegation_requester: requester.value(),
        delegated_account_bump,
        seed_count: u8::from(!encoded_seeds.is_empty()),
        encoded_seeds,
    }
    .encode()
    .unwrap();
    let state_rent = Rent::default().minimum_balance(state_data.len());
    program_test.add_account(
        delegation_state,
        Account {
            lamports: state_rent,
            data: state_data,
            owner: dpi::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks, payer, _) = program_test.start().await;
    UndelegateTestContext {
        banks,
        payer,
        current_validator,
        delegated_account,
        delegation_state,
        delegation_cluster,
        delegation_rent_payer,
        original_owner,
        state_rent,
    }
}

fn owner_program_processor(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let expected_callback_data = [
        EXTERNAL_UNDELEGATE_DISCRIMINATOR.as_slice(),
        1_u32.to_le_bytes().as_slice(),
        (ACCOUNT_SEED.len() as u32).to_le_bytes().as_slice(),
        ACCOUNT_SEED,
    ]
    .concat();
    if program_id != &OWNER_PROGRAM_ID || data != expected_callback_data {
        return Err(ProgramError::InvalidInstructionData);
    }

    let mut accounts = accounts.iter();
    let delegated_account = next_account_info(&mut accounts)?;
    let undelegate_buffer = next_account_info(&mut accounts)?;
    let payer = next_account_info(&mut accounts)?;
    let system_program_account = next_account_info(&mut accounts)?;
    if delegated_account.owner != program_id
        || !delegated_account.is_writable
        || undelegate_buffer.owner != &dpi::id()
        || !undelegate_buffer.is_signer
        || !payer.is_signer
        || system_program_account.key != &system_program::id()
    {
        return Err(ProgramError::InvalidAccountData);
    }

    let buffer_data = undelegate_buffer.try_borrow_data()?;
    let mut delegated_data = delegated_account.try_borrow_mut_data()?;
    if delegated_data.len() != buffer_data.len() {
        return Err(ProgramError::InvalidAccountData);
    }
    delegated_data.copy_from_slice(&buffer_data);
    Ok(())
}

fn assert_custom_error(
    error: BanksClientError,
    expected: dpi::error::DlpError,
) {
    assert!(
        matches!(
            error,
            BanksClientError::TransactionError(
                TransactionError::InstructionError(
                    0,
                    InstructionError::Custom(code),
                ),
            ) if code == expected as u32
        ),
        "unexpected error: {error:?}"
    );
}
