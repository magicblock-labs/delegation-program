use dlp::solana_program;
use dlp_api::{
    consts::DEFAULT_UNDELEGATION_REQUEST_TIMEOUT_SLOTS,
    pda::{
        commit_record_pda_from_delegated_account,
        commit_state_pda_from_delegated_account,
        delegation_metadata_pda_from_delegated_account,
        delegation_record_pda_from_delegated_account, fees_vault_pda,
        undelegation_request_pda_from_delegated_account,
        validator_fees_vault_pda_from_validator,
    },
    state::{DelegationMetadata, UndelegationRequest, UndelegationRequester},
};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    hash::Hash,
    instruction::{AccountMeta, Instruction},
    native_token::LAMPORTS_PER_SOL,
    program::invoke_signed,
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{
    processor, read_file, BanksClient, BanksClientError, ProgramTest,
};
use solana_sdk::{
    account::Account,
    instruction::InstructionError,
    signature::{Keypair, Signer},
    transaction::{Transaction, TransactionError},
};
use solana_sdk_ids::system_program;

use crate::fixtures::{
    create_undelegation_request_data, get_commit_record_account_data,
    get_delegation_metadata_data, get_delegation_metadata_data_on_curve,
    get_delegation_record_data, get_delegation_record_on_curve_data,
    keypair_from_bytes, COMMIT_NEW_STATE_ACCOUNT_DATA, DELEGATED_PDA_ID,
    DELEGATED_PDA_OWNER_ID, ON_CURVE_KEYPAIR, TEST_AUTHORITY,
};

mod fixtures;

const TEST_PDA_SEED: &[u8] = b"test-pda";

#[tokio::test]
async fn test_request_undelegation_creates_request() {
    let (banks, payer, _, blockhash) = setup_request_env().await;

    let ix = request_undelegation_from_owner_program(payer.pubkey());
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer],
        blockhash,
    );

    let res = banks.process_transaction(tx).await;
    println!("{:?}", res);
    assert!(res.is_ok());

    let request_pda =
        undelegation_request_pda_from_delegated_account(&DELEGATED_PDA_ID);
    let request_account =
        banks.get_account(request_pda).await.unwrap().unwrap();
    assert_eq!(request_account.owner, dlp_api::id());

    let request = UndelegationRequest::try_from_bytes_with_discriminator(
        &request_account.data,
    )
    .unwrap();
    assert_eq!(request.delegated_account, DELEGATED_PDA_ID);
    assert_eq!(request.owner_program, DELEGATED_PDA_OWNER_ID);
    assert_eq!(request.rent_payer, payer.pubkey());
    assert_eq!(
        request.expires_at_slot,
        request.created_slot + DEFAULT_UNDELEGATION_REQUEST_TIMEOUT_SLOTS
    );
    assert_eq!(request.last_commit_nonce_at_request, 0);

    let delegation_metadata_pda =
        delegation_metadata_pda_from_delegated_account(&DELEGATED_PDA_ID);
    let delegation_metadata_account = banks
        .get_account(delegation_metadata_pda)
        .await
        .unwrap()
        .unwrap();
    let delegation_metadata =
        DelegationMetadata::try_from_bytes_with_discriminator(
            &delegation_metadata_account.data,
        )
        .unwrap();
    assert_eq!(
        delegation_metadata.undelegatable,
        UndelegationRequester::OwnerProgram
    );
}

#[tokio::test]
async fn test_request_undelegation_is_idempotent() {
    let (banks, payer, second_payer, blockhash) = setup_request_env().await;

    let first_ix = request_undelegation_from_owner_program(payer.pubkey());
    let first_tx = Transaction::new_signed_with_payer(
        &[first_ix],
        Some(&payer.pubkey()),
        &[&payer],
        blockhash,
    );
    let first_res = banks.process_transaction(first_tx).await;
    assert!(first_res.is_ok());

    let request_pda =
        undelegation_request_pda_from_delegated_account(&DELEGATED_PDA_ID);
    let request_before = banks.get_account(request_pda).await.unwrap().unwrap();

    let second_ix =
        request_undelegation_from_owner_program(second_payer.pubkey());
    let second_tx = Transaction::new_signed_with_payer(
        &[second_ix],
        Some(&second_payer.pubkey()),
        &[&second_payer],
        blockhash,
    );
    let second_res = banks.process_transaction(second_tx).await;
    println!("{:?}", second_res);
    assert!(second_res.is_ok());

    let request_after = banks.get_account(request_pda).await.unwrap().unwrap();
    assert_eq!(request_after.data, request_before.data);
}

#[tokio::test]
async fn test_request_undelegation_rejects_payload() {
    let (banks, payer, _, blockhash) = setup_request_env().await;

    let mut ix = request_undelegation_from_owner_program(payer.pubkey());
    ix.data = 123_u64.to_le_bytes().to_vec();
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer],
        blockhash,
    );

    let err = banks.process_transaction(tx).await.unwrap_err();
    assert!(
        matches!(
            err,
            BanksClientError::TransactionError(
                TransactionError::InstructionError(
                    0,
                    InstructionError::InvalidInstructionData,
                )
            )
        ),
        "expected request payload to fail with InvalidInstructionData, got {err:?}"
    );
}

#[tokio::test]
async fn test_request_undelegation_rejects_missing_delegated_signer() {
    let (banks, payer, _, blockhash) = setup_request_env().await;

    let request_pda =
        undelegation_request_pda_from_delegated_account(&DELEGATED_PDA_ID);
    let ix = Instruction {
        program_id: dlp_api::id(),
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(DELEGATED_PDA_ID, false),
            AccountMeta::new_readonly(DELEGATED_PDA_OWNER_ID, false),
            AccountMeta::new(request_pda, false),
            AccountMeta::new_readonly(
                delegation_record_pda_from_delegated_account(&DELEGATED_PDA_ID),
                false,
            ),
            AccountMeta::new(
                delegation_metadata_pda_from_delegated_account(
                    &DELEGATED_PDA_ID,
                ),
                false,
            ),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: dlp_api::discriminator::DlpDiscriminator::RequestUndelegation
            .to_vec(),
    };

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    assert!(res.is_err());
}

#[tokio::test]
async fn test_request_undelegation_rejects_on_curve_delegated_account() {
    let (banks, payer, delegated_on_curve, blockhash) =
        setup_on_curve_request_env().await;

    let ix = dlp_api::instruction_builder::request_undelegation(
        payer.pubkey(),
        delegated_on_curve.pubkey(),
        system_program::id(),
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &delegated_on_curve],
        blockhash,
    );

    let res = banks.process_transaction(tx).await;
    assert!(res.is_err());
}

#[tokio::test]
async fn test_undelegate_with_request_closes_request() {
    let (banks, _, authority, request_rent_payer, blockhash) =
        setup_undelegate_with_request_env().await;

    let request_pda =
        undelegation_request_pda_from_delegated_account(&DELEGATED_PDA_ID);
    let request_lamports_before = banks
        .get_account(request_pda)
        .await
        .unwrap()
        .unwrap()
        .lamports;
    let payer_lamports_before = banks
        .get_account(request_rent_payer.pubkey())
        .await
        .unwrap()
        .unwrap()
        .lamports;

    let ix_finalize = dlp_api::instruction_builder::finalize(
        authority.pubkey(),
        DELEGATED_PDA_ID,
    );
    let ix_undelegate = dlp_api::instruction_builder::undelegate_with_request(
        authority.pubkey(),
        DELEGATED_PDA_ID,
        DELEGATED_PDA_OWNER_ID,
        authority.pubkey(),
        request_rent_payer.pubkey(),
    );

    let tx = Transaction::new_signed_with_payer(
        &[ix_finalize, ix_undelegate],
        Some(&authority.pubkey()),
        &[&authority],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    println!("{:?}", res);
    assert!(res.is_ok());

    let request_account = banks.get_account(request_pda).await.unwrap();
    assert!(request_account.is_none());

    let payer_lamports_after = banks
        .get_account(request_rent_payer.pubkey())
        .await
        .unwrap()
        .unwrap()
        .lamports;
    assert_eq!(
        payer_lamports_after,
        payer_lamports_before + request_lamports_before
    );
}

#[tokio::test]
async fn test_undelegate_with_malformed_optional_request_accounts_rejected() {
    let (banks, _, authority, request_rent_payer, blockhash) =
        setup_undelegate_with_request_env().await;

    let ix_finalize = dlp_api::instruction_builder::finalize(
        authority.pubkey(),
        DELEGATED_PDA_ID,
    );
    let mut ix_undelegate =
        dlp_api::instruction_builder::undelegate_with_request(
            authority.pubkey(),
            DELEGATED_PDA_ID,
            DELEGATED_PDA_OWNER_ID,
            authority.pubkey(),
            request_rent_payer.pubkey(),
        );
    ix_undelegate.accounts.pop();

    let tx = Transaction::new_signed_with_payer(
        &[ix_finalize, ix_undelegate],
        Some(&authority.pubkey()),
        &[&authority],
        blockhash,
    );
    let err = banks.process_transaction(tx).await.unwrap_err();
    assert!(
        matches!(
            err,
            BanksClientError::TransactionError(
                TransactionError::InstructionError(
                    1,
                    InstructionError::InvalidInstructionData,
                )
            )
        ),
        "expected malformed optional request accounts to fail undelegate \
         with InvalidInstructionData, got {err:?}"
    );
}

fn imaginary_program_processor_requesting_undelegation_through_cpi(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let [payer, delegated_account, owner_program, request_account, delegation_record_account, delegation_metadata_account, system_program_account, dlp_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if owner_program.key != program_id {
        return Err(ProgramError::IncorrectProgramId);
    }

    let mut ix = dlp_api::instruction_builder::request_undelegation(
        *payer.key,
        *delegated_account.key,
        *program_id,
    );
    ix.data.extend_from_slice(data);
    let (_, bump) = Pubkey::find_program_address(&[TEST_PDA_SEED], program_id);
    let bump_seed = [bump];
    invoke_signed(
        &ix,
        &[
            payer.clone(),
            delegated_account.clone(),
            owner_program.clone(),
            request_account.clone(),
            delegation_record_account.clone(),
            delegation_metadata_account.clone(),
            system_program_account.clone(),
            dlp_program.clone(),
        ],
        &[&[TEST_PDA_SEED, &bump_seed]],
    )
}

///
/// Note this instruction invokes an imaginary "owner program" which then calls
/// the DLP program to request undelegation, which is why "data" doesn't contain
/// any "instruction discriminator" because the imaginary program doesn't
/// require it. See imaginary_program_processor_requesting_undelegation_through_cpi()
/// which is supposed to be the processor of the imaginary program.
///
fn request_undelegation_from_owner_program(payer: Pubkey) -> Instruction {
    Instruction {
        program_id: DELEGATED_PDA_OWNER_ID,
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(DELEGATED_PDA_ID, false),
            AccountMeta::new_readonly(DELEGATED_PDA_OWNER_ID, false),
            AccountMeta::new(
                undelegation_request_pda_from_delegated_account(
                    &DELEGATED_PDA_ID,
                ),
                false,
            ),
            AccountMeta::new_readonly(
                delegation_record_pda_from_delegated_account(&DELEGATED_PDA_ID),
                false,
            ),
            AccountMeta::new(
                delegation_metadata_pda_from_delegated_account(
                    &DELEGATED_PDA_ID,
                ),
                false,
            ),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(dlp_api::id(), false),
        ],
        data: vec![],
    }
}

async fn setup_request_env() -> (BanksClient, Keypair, Keypair, Hash) {
    let mut program_test = ProgramTest::default();
    program_test.prefer_bpf(true);
    program_test.add_program("dlp", dlp_api::ID, None);
    program_test.prefer_bpf(false);
    program_test.add_program(
        "request-wrapper",
        DELEGATED_PDA_OWNER_ID,
        processor!(
            imaginary_program_processor_requesting_undelegation_through_cpi
        ),
    );
    program_test.prefer_bpf(true);

    let payer = Keypair::new();
    let second_payer = Keypair::new();
    let authority = keypair_from_bytes(&TEST_AUTHORITY);

    add_system_account(&mut program_test, payer.pubkey(), LAMPORTS_PER_SOL);
    add_system_account(
        &mut program_test,
        second_payer.pubkey(),
        LAMPORTS_PER_SOL,
    );
    add_system_account(&mut program_test, authority.pubkey(), LAMPORTS_PER_SOL);

    add_delegated_account(&mut program_test, DELEGATED_PDA_ID);
    add_delegation_accounts(&mut program_test, authority.pubkey());

    let (banks, _, blockhash) = program_test.start().await;
    (banks, payer, second_payer, blockhash)
}

async fn setup_on_curve_request_env() -> (BanksClient, Keypair, Keypair, Hash) {
    let mut program_test = ProgramTest::new("dlp", dlp_api::ID, None);
    program_test.prefer_bpf(true);
    let payer = Keypair::new();
    let delegated_on_curve = keypair_from_bytes(&ON_CURVE_KEYPAIR);

    add_system_account(&mut program_test, payer.pubkey(), LAMPORTS_PER_SOL);
    program_test.add_account(
        delegated_on_curve.pubkey(),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: dlp_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let delegation_record_data = get_delegation_record_on_curve_data(
        delegated_on_curve.pubkey(),
        Some(LAMPORTS_PER_SOL),
    );
    program_test.add_account(
        delegation_record_pda_from_delegated_account(
            &delegated_on_curve.pubkey(),
        ),
        Account {
            lamports: Rent::default()
                .minimum_balance(delegation_record_data.len()),
            data: delegation_record_data,
            owner: dlp_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
    let delegation_metadata_data =
        get_delegation_metadata_data_on_curve(payer.pubkey(), Some(false));
    program_test.add_account(
        delegation_metadata_pda_from_delegated_account(
            &delegated_on_curve.pubkey(),
        ),
        Account {
            lamports: Rent::default()
                .minimum_balance(delegation_metadata_data.len()),
            data: delegation_metadata_data,
            owner: dlp_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks, _, blockhash) = program_test.start().await;
    (banks, payer, delegated_on_curve, blockhash)
}

async fn setup_undelegate_with_request_env(
) -> (BanksClient, Keypair, Keypair, Keypair, Hash) {
    let mut program_test = ProgramTest::new("dlp", dlp_api::ID, None);
    program_test.prefer_bpf(true);
    let authority = keypair_from_bytes(&TEST_AUTHORITY);
    let request_rent_payer = Keypair::new();

    add_system_account(&mut program_test, authority.pubkey(), LAMPORTS_PER_SOL);
    add_system_account(
        &mut program_test,
        request_rent_payer.pubkey(),
        LAMPORTS_PER_SOL,
    );

    add_delegated_account(&mut program_test, DELEGATED_PDA_ID);
    add_delegation_accounts_with_metadata(
        &mut program_test,
        authority.pubkey(),
        Some(true),
    );
    add_commit_accounts(&mut program_test, authority.pubkey());
    add_owner_program(&mut program_test);
    add_fee_accounts(&mut program_test, authority.pubkey());
    add_request_account(&mut program_test, request_rent_payer.pubkey());

    let (banks, payer, blockhash) = program_test.start().await;
    (banks, payer, authority, request_rent_payer, blockhash)
}

fn add_system_account(
    program_test: &mut ProgramTest,
    pubkey: Pubkey,
    lamports: u64,
) {
    program_test.add_account(
        pubkey,
        Account {
            lamports,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
}

fn add_delegated_account(program_test: &mut ProgramTest, pubkey: Pubkey) {
    program_test.add_account(
        pubkey,
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: dlp_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
}

fn add_delegation_accounts(program_test: &mut ProgramTest, authority: Pubkey) {
    add_delegation_accounts_with_metadata(program_test, authority, Some(false));
}

fn add_delegation_accounts_with_metadata(
    program_test: &mut ProgramTest,
    authority: Pubkey,
    undelegatable: Option<bool>,
) {
    let delegation_record_data = get_delegation_record_data(authority, None);
    program_test.add_account(
        delegation_record_pda_from_delegated_account(&DELEGATED_PDA_ID),
        Account {
            lamports: Rent::default()
                .minimum_balance(delegation_record_data.len()),
            data: delegation_record_data,
            owner: dlp_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let delegation_metadata_data =
        get_delegation_metadata_data(authority, undelegatable);
    program_test.add_account(
        delegation_metadata_pda_from_delegated_account(&DELEGATED_PDA_ID),
        Account {
            lamports: Rent::default()
                .minimum_balance(delegation_metadata_data.len()),
            data: delegation_metadata_data,
            owner: dlp_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
}

fn add_commit_accounts(program_test: &mut ProgramTest, authority: Pubkey) {
    program_test.add_account(
        commit_state_pda_from_delegated_account(&DELEGATED_PDA_ID),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: COMMIT_NEW_STATE_ACCOUNT_DATA.into(),
            owner: dlp_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let commit_record_data = get_commit_record_account_data(authority);
    program_test.add_account(
        commit_record_pda_from_delegated_account(&DELEGATED_PDA_ID),
        Account {
            lamports: Rent::default().minimum_balance(commit_record_data.len()),
            data: commit_record_data,
            owner: dlp_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
}

fn add_owner_program(program_test: &mut ProgramTest) {
    let data = read_file("tests/buffers/test_delegation.so");
    program_test.add_account(
        DELEGATED_PDA_OWNER_ID,
        Account {
            lamports: Rent::default().minimum_balance(data.len()),
            data,
            owner: solana_sdk::bpf_loader::id(),
            executable: true,
            rent_epoch: 0,
        },
    );
}

fn add_fee_accounts(program_test: &mut ProgramTest, authority: Pubkey) {
    program_test.add_account(
        fees_vault_pda(),
        Account {
            lamports: Rent::default().minimum_balance(0),
            data: vec![],
            owner: dlp_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
    program_test.add_account(
        validator_fees_vault_pda_from_validator(&authority),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: dlp_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
}

fn add_request_account(program_test: &mut ProgramTest, rent_payer: Pubkey) {
    let request_data = create_undelegation_request_data(
        DELEGATED_PDA_ID,
        DELEGATED_PDA_OWNER_ID,
        rent_payer,
        1,
    );
    program_test.add_account(
        undelegation_request_pda_from_delegated_account(&DELEGATED_PDA_ID),
        Account {
            lamports: Rent::default().minimum_balance(request_data.len()),
            data: request_data,
            owner: dlp_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
}
