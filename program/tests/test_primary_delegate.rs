use delegation_program::solana_program;
use delegation_program_interface as dpi;
use dpi::{
    instruction_builder::delegate,
    legacy::pda::DELEGATE_BUFFER_TAG,
    pda::{delegation_state_pda, DELEGATION_STATE_SEED},
    DelegateArgs, DelegationSeed, DelegationState, UndelegationRequester,
};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    instruction::{AccountMeta, Instruction},
    native_token::LAMPORTS_PER_SOL,
    program::invoke_signed,
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
use wheels::layout::Decodable;

#[allow(dead_code)]
mod fixtures;

use crate::fixtures::v2::{direct_cluster_args, initialize_delegation_cluster};

const OWNER_PROGRAM_ID: Pubkey =
    solana_sdk::pubkey!("4vJ9JU1bJJE96FWSJKvHsmmFADCg4gpZQff4EVRtnYHk");
const DELEGATED_ACCOUNT_SEED: &[u8] = b"primary-delegated";
const DELEGATED_ACCOUNT_DATA: &[u8] = b"primary account data";
const DELEGATE_BUFFER_DATA: &[u8] = b"delegate buffer data";

struct DelegateTestContext {
    banks: BanksClient,
    payer: Keypair,
    delegated_account: Pubkey,
    delegated_account_bump: u8,
    delegation_cluster: Pubkey,
}

#[tokio::test]
async fn test_delegate_creates_cluster_bound_delegation_state() {
    let context = setup_delegate_test().await;
    let delegation_state =
        delegation_state_pda(&context.delegated_account.to_bytes().into());
    let (_, expected_state_bump) = Pubkey::find_program_address(
        &[DELEGATION_STATE_SEED, context.delegated_account.as_ref()],
        &dpi::id(),
    );

    let result = submit_delegate(
        &context,
        DelegateArgs::new(
            context.delegated_account_bump,
            vec![DelegationSeed {
                value: DELEGATED_ACCOUNT_SEED.to_vec(),
            }],
        ),
    )
    .await;
    assert!(result.is_ok());

    let delegated_account = context
        .banks
        .get_account(context.delegated_account)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(delegated_account.owner, dpi::id());
    assert_eq!(delegated_account.data, DELEGATE_BUFFER_DATA);

    let state_account = context
        .banks
        .get_account(delegation_state)
        .await
        .unwrap()
        .unwrap();
    let state = DelegationState::decode(&state_account.data).unwrap();
    let seeds = state.seeds().unwrap();

    assert_eq!(state.discriminator(), DelegationState::DISCRIMINATOR);
    assert_eq!(state.bump(), expected_state_bump);
    assert_eq!(*state.delegated_account(), context.delegated_account);
    assert_eq!(*state.delegation_cluster(), context.delegation_cluster);
    assert_eq!(*state.original_owner(), OWNER_PROGRAM_ID);
    assert_eq!(*state.rent_payer(), context.payer.pubkey());
    assert_eq!(state.lamports(), delegated_account.lamports);
    assert_eq!(state.last_commit_id(), 0);
    assert_eq!(
        state.undelegation_requester(),
        UndelegationRequester::None.value()
    );
    assert_eq!(
        state.delegated_account_bump(),
        context.delegated_account_bump
    );
    assert_eq!(seeds.len(), 1);
    assert_eq!(seeds.get(0).unwrap().as_slice(), DELEGATED_ACCOUNT_SEED);
    assert_eq!(
        state_account.lamports,
        Rent::default().minimum_balance(state_account.data.len())
    );
}

#[tokio::test]
async fn test_delegate_rejects_wrong_delegated_account_seeds() {
    let context = setup_delegate_test().await;
    let result = submit_delegate(
        &context,
        DelegateArgs::new(
            context.delegated_account_bump,
            vec![DelegationSeed {
                value: b"wrong-seed".to_vec(),
            }],
        ),
    )
    .await;

    assert!(
        matches!(
            &result,
            Err(BanksClientError::TransactionError(
                TransactionError::InstructionError(
                    0,
                    InstructionError::InvalidSeeds
                )
            ))
        ),
        "unexpected result: {result:?}"
    );
    assert!(context
        .banks
        .get_account(delegation_state_pda(
            &context.delegated_account.to_bytes().into()
        ))
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn test_delegate_rejects_non_system_owner_for_on_curve_account() {
    let mut program_test = ProgramTest::default();
    program_test.prefer_bpf(true);
    program_test.add_program("delegation_program", dpi::ID, None);

    let delegated_account = Keypair::new();
    let config_authority = Keypair::new();
    program_test.add_account(
        delegated_account.pubkey(),
        Account {
            lamports: Rent::default().minimum_balance(0),
            data: vec![],
            owner: dpi::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
    program_test.add_account(
        config_authority.pubkey(),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks, payer, blockhash) = program_test.start().await;
    let delegation_cluster = initialize_delegation_cluster(
        &banks,
        &payer,
        &config_authority,
        blockhash,
        direct_cluster_args(Pubkey::new_unique()),
    )
    .await;
    let instruction = delegate(
        payer.pubkey(),
        delegated_account.pubkey(),
        OWNER_PROGRAM_ID,
        delegation_cluster,
        DelegateArgs::new(0, vec![]),
    )
    .unwrap();
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &[&payer, &delegated_account],
        banks.get_latest_blockhash().await.unwrap(),
    );
    let result = banks.process_transaction(transaction).await;

    assert!(
        matches!(
            &result,
            Err(BanksClientError::TransactionError(
                TransactionError::InstructionError(
                    0,
                    InstructionError::IncorrectProgramId
                )
            ))
        ),
        "unexpected result: {result:?}"
    );
}

async fn setup_delegate_test() -> DelegateTestContext {
    let mut program_test = ProgramTest::default();
    program_test.prefer_bpf(true);
    program_test.add_program("delegation_program", dpi::ID, None);
    program_test.prefer_bpf(false);
    program_test.add_program(
        "primary-delegate-owner",
        OWNER_PROGRAM_ID,
        processor!(owner_program_processor),
    );
    program_test.prefer_bpf(true);

    let (delegated_account, delegated_account_bump) =
        Pubkey::find_program_address(
            &[DELEGATED_ACCOUNT_SEED],
            &OWNER_PROGRAM_ID,
        );
    let delegate_buffer = Pubkey::find_program_address(
        &[DELEGATE_BUFFER_TAG, delegated_account.as_ref()],
        &OWNER_PROGRAM_ID,
    )
    .0;
    let config_authority = Keypair::new();

    program_test.add_account(
        delegated_account,
        Account {
            lamports: Rent::default()
                .minimum_balance(DELEGATED_ACCOUNT_DATA.len()),
            data: DELEGATED_ACCOUNT_DATA.to_vec(),
            owner: dpi::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
    program_test.add_account(
        delegate_buffer,
        Account {
            lamports: Rent::default()
                .minimum_balance(DELEGATE_BUFFER_DATA.len()),
            data: DELEGATE_BUFFER_DATA.to_vec(),
            owner: OWNER_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    program_test.add_account(
        config_authority.pubkey(),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks, payer, blockhash) = program_test.start().await;
    let delegation_cluster = initialize_delegation_cluster(
        &banks,
        &payer,
        &config_authority,
        blockhash,
        direct_cluster_args(Pubkey::new_unique()),
    )
    .await;

    DelegateTestContext {
        banks,
        payer,
        delegated_account,
        delegated_account_bump,
        delegation_cluster,
    }
}

async fn submit_delegate(
    context: &DelegateTestContext,
    args: DelegateArgs,
) -> Result<(), solana_program_test::BanksClientError> {
    let inner = delegate(
        context.payer.pubkey(),
        context.delegated_account,
        OWNER_PROGRAM_ID,
        context.delegation_cluster,
        args,
    )
    .unwrap();
    let mut accounts = inner.accounts;
    accounts[1].is_signer = false;
    accounts.push(AccountMeta::new_readonly(dpi::id(), false));
    let outer = Instruction {
        program_id: OWNER_PROGRAM_ID,
        accounts,
        data: inner.data,
    };
    let transaction = Transaction::new_signed_with_payer(
        &[outer],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        context.banks.get_latest_blockhash().await.unwrap(),
    );

    context.banks.process_transaction(transaction).await
}

fn owner_program_processor(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let [payer, delegated_account, original_owner, delegate_buffer, delegation_state, delegation_cluster, system_program, dlp_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if original_owner.key != program_id || dlp_program.key != &dpi::id() {
        return Err(ProgramError::IncorrectProgramId);
    }

    let instruction = Instruction {
        program_id: dpi::id(),
        accounts: vec![
            AccountMeta::new(*payer.key, true),
            AccountMeta::new(*delegated_account.key, true),
            AccountMeta::new_readonly(*original_owner.key, false),
            AccountMeta::new(*delegate_buffer.key, false),
            AccountMeta::new(*delegation_state.key, false),
            AccountMeta::new_readonly(*delegation_cluster.key, false),
            AccountMeta::new_readonly(*system_program.key, false),
        ],
        data: data.to_vec(),
    };
    let delegated_account_bump =
        [
            Pubkey::find_program_address(&[DELEGATED_ACCOUNT_SEED], program_id)
                .1,
        ];

    invoke_signed(
        &instruction,
        accounts,
        &[&[DELEGATED_ACCOUNT_SEED, &delegated_account_bump]],
    )
}
