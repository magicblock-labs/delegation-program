use dlp::solana_program;
use dlp_api::{
    args::{CommitFinalizeArgs, CommitStateArgs},
    consts::DEFAULT_UNDELEGATION_REQUEST_TIMEOUT_SLOTS,
    error::DlpError,
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
    let SetupContext {
        banks,
        authority,
        blockhash,
        ..
    } = setup_env(SetupConfig {
        with_request_wrapper: true,
        ..Default::default()
    })
    .await;

    let ix = request_undelegation_from_owner_program(authority.pubkey());
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&authority.pubkey()),
        &[&authority],
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
    assert_eq!(request.rent_payer, authority.pubkey());
    assert_eq!(
        request.expires_at_slot,
        request.created_slot + DEFAULT_UNDELEGATION_REQUEST_TIMEOUT_SLOTS
    );
    assert_eq!(request.last_commit_id_at_request, 0);

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
        delegation_metadata.undelegation_requester,
        UndelegationRequester::OwnerProgram
    );
}

#[tokio::test]
async fn test_request_undelegation_is_idempotent() {
    let SetupContext {
        banks,
        authority,
        blockhash,
        ..
    } = setup_env(SetupConfig {
        with_request_wrapper: true,
        ..Default::default()
    })
    .await;

    let first_ix = request_undelegation_from_owner_program(authority.pubkey());
    let first_tx = Transaction::new_signed_with_payer(
        &[first_ix],
        Some(&authority.pubkey()),
        &[&authority],
        blockhash,
    );
    let first_res = banks.process_transaction(first_tx).await;
    assert!(first_res.is_ok());

    let request_pda =
        undelegation_request_pda_from_delegated_account(&DELEGATED_PDA_ID);
    let request_before = banks.get_account(request_pda).await.unwrap().unwrap();

    let second_ix = request_undelegation_from_owner_program(authority.pubkey());
    let second_tx = Transaction::new_signed_with_payer(
        &[second_ix],
        Some(&authority.pubkey()),
        &[&authority],
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
    let SetupContext {
        banks,
        payer,
        blockhash,
        ..
    } = setup_env(SetupConfig {
        with_request_wrapper: true,
        ..Default::default()
    })
    .await;

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
    let SetupContext {
        banks,
        payer,
        blockhash,
        ..
    } = setup_env(SetupConfig {
        with_request_wrapper: true,
        ..Default::default()
    })
    .await;

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
    let SetupContext {
        banks,
        payer,
        delegated_on_curve,
        delegated_account,
        blockhash,
        ..
    } = setup_env(SetupConfig {
        delegated_account: DelegatedAccountSetup::OnCurveKeypair,
        ..Default::default()
    })
    .await;

    let ix = dlp_api::instruction_builder::request_undelegation(
        payer.pubkey(),
        delegated_account,
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
    let SetupContext {
        banks,
        authority,
        blockhash,
        ..
    } = setup_env(SetupConfig {
        metadata_undelegatable: true,
        with_commit_accounts: true,
        with_owner_program: true,
        with_fee_accounts: true,
        with_request_account: true,
        ..Default::default()
    })
    .await;

    let request_pda =
        undelegation_request_pda_from_delegated_account(&DELEGATED_PDA_ID);

    let ix_finalize = dlp_api::instruction_builder::finalize(
        authority.pubkey(),
        DELEGATED_PDA_ID,
    );
    let ix_undelegate = dlp_api::instruction_builder::undelegate_with_request(
        authority.pubkey(),
        DELEGATED_PDA_ID,
        DELEGATED_PDA_OWNER_ID,
        authority.pubkey(),
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
}

#[tokio::test]
async fn test_commit_state_preserves_owner_program_requester() {
    let SetupContext {
        banks,
        authority,
        blockhash,
        ..
    } = setup_env(SetupConfig {
        with_request_wrapper: true,
        with_fee_accounts: true,
        ..Default::default()
    })
    .await;

    let request_ix =
        request_undelegation_from_owner_program(authority.pubkey());
    let commit_ix = dlp_api::instruction_builder::commit_state(
        authority.pubkey(),
        DELEGATED_PDA_ID,
        DELEGATED_PDA_OWNER_ID,
        CommitStateArgs {
            data: COMMIT_NEW_STATE_ACCOUNT_DATA.to_vec(),
            nonce: 1,
            allow_undelegation: false,
            lamports: LAMPORTS_PER_SOL,
        },
    );
    let tx = Transaction::new_signed_with_payer(
        &[request_ix, commit_ix],
        Some(&authority.pubkey()),
        &[&authority],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok(), "{res:?}");

    assert_delegation_metadata_requester(
        &banks,
        UndelegationRequester::OwnerProgram,
    )
    .await;
}

#[tokio::test]
async fn test_commit_finalize_preserves_owner_program_requester() {
    let SetupContext {
        banks,
        authority,
        blockhash,
        ..
    } = setup_env(SetupConfig {
        with_request_wrapper: true,
        with_fee_accounts: true,
        ..Default::default()
    })
    .await;

    let request_ix =
        request_undelegation_from_owner_program(authority.pubkey());
    let mut args = CommitFinalizeArgs {
        commit_id: 1,
        lamports: LAMPORTS_PER_SOL,
        allow_undelegation: false.into(),
        data_is_diff: false.into(),
        bumps: Default::default(),
        reserved_padding: Default::default(),
    };
    let (commit_finalize_ix, _) = dlp_api::instruction_builder::commit_finalize(
        authority.pubkey(),
        DELEGATED_PDA_ID,
        &mut args,
        &COMMIT_NEW_STATE_ACCOUNT_DATA,
    );
    let tx = Transaction::new_signed_with_payer(
        &[request_ix, commit_finalize_ix],
        Some(&authority.pubkey()),
        &[&authority],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    assert!(res.is_ok(), "{res:?}");

    assert_delegation_metadata_requester(
        &banks,
        UndelegationRequester::OwnerProgram,
    )
    .await;
}

#[tokio::test]
async fn test_undelegate_owner_program_request_without_request_accounts_rejected(
) {
    let SetupContext {
        banks,
        authority,
        blockhash,
        ..
    } = setup_env(SetupConfig {
        with_request_wrapper: true,
        with_commit_accounts: true,
        with_fee_accounts: true,
        ..Default::default()
    })
    .await;

    let request_ix =
        request_undelegation_from_owner_program(authority.pubkey());
    let request_tx = Transaction::new_signed_with_payer(
        &[request_ix],
        Some(&authority.pubkey()),
        &[&authority],
        blockhash,
    );
    let request_res = banks.process_transaction(request_tx).await;
    assert!(request_res.is_ok(), "{request_res:?}");

    let finalize_ix = dlp_api::instruction_builder::finalize(
        authority.pubkey(),
        DELEGATED_PDA_ID,
    );
    let finalize_tx = Transaction::new_signed_with_payer(
        &[finalize_ix],
        Some(&authority.pubkey()),
        &[&authority],
        blockhash,
    );
    let finalize_res = banks.process_transaction(finalize_tx).await;
    assert!(finalize_res.is_ok(), "{finalize_res:?}");

    let undelegate_ix = dlp_api::instruction_builder::undelegate(
        authority.pubkey(),
        DELEGATED_PDA_ID,
        DELEGATED_PDA_OWNER_ID,
        authority.pubkey(),
    );
    let undelegate_tx = Transaction::new_signed_with_payer(
        &[undelegate_ix],
        Some(&authority.pubkey()),
        &[&authority],
        blockhash,
    );
    let err = banks.process_transaction(undelegate_tx).await.unwrap_err();
    assert!(
        matches!(
            err,
            BanksClientError::TransactionError(
                TransactionError::InstructionError(
                    0,
                    InstructionError::Custom(code),
                )
            ) if code == DlpError::MissingUndelegationRequest as u32
        ),
        "expected MissingUndelegationRequest, got {err:?}"
    );
}

async fn assert_delegation_metadata_requester(
    banks: &BanksClient,
    expected_requester: UndelegationRequester,
) {
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
        delegation_metadata.undelegation_requester,
        expected_requester
    );
}

#[tokio::test]
async fn test_undelegate_with_malformed_optional_request_accounts_rejected() {
    let SetupContext {
        banks,
        authority,
        blockhash,
        ..
    } = setup_env(SetupConfig {
        metadata_undelegatable: true,
        with_commit_accounts: true,
        with_owner_program: true,
        with_fee_accounts: true,
        with_request_account: true,
        ..Default::default()
    })
    .await;

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
        );
    ix_undelegate
        .accounts
        .push(AccountMeta::new_readonly(system_program::id(), false));
    ix_undelegate
        .accounts
        .push(AccountMeta::new_readonly(dlp_api::id(), false));

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
fn request_undelegation_from_owner_program(
    delegation_rent_payer: Pubkey,
) -> Instruction {
    Instruction {
        program_id: DELEGATED_PDA_OWNER_ID,
        accounts: vec![
            AccountMeta::new(delegation_rent_payer, true),
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

#[derive(Clone, Copy)]
enum DelegatedAccountSetup {
    OffCurvePda,
    OnCurveKeypair,
}

impl Default for DelegatedAccountSetup {
    fn default() -> Self {
        Self::OffCurvePda
    }
}

#[derive(Default)]
struct SetupConfig {
    delegated_account: DelegatedAccountSetup,
    with_request_wrapper: bool,
    metadata_undelegatable: bool,
    with_commit_accounts: bool,
    with_owner_program: bool,
    with_fee_accounts: bool,
    with_request_account: bool,
}

struct SetupContext {
    banks: BanksClient,
    payer: Keypair,
    authority: Keypair,
    delegated_on_curve: Keypair,
    delegated_account: Pubkey,
    blockhash: Hash,
}

async fn setup_env(config: SetupConfig) -> SetupContext {
    let mut program_test = ProgramTest::new("dlp", dlp_api::ID, None);
    program_test.prefer_bpf(true);
    if config.with_request_wrapper {
        program_test.prefer_bpf(false);
        program_test.add_program(
            "request-wrapper",
            DELEGATED_PDA_OWNER_ID,
            processor!(
                imaginary_program_processor_requesting_undelegation_through_cpi
            ),
        );
        program_test.prefer_bpf(true);
    }

    let payer = Keypair::new();
    let authority = keypair_from_bytes(&TEST_AUTHORITY);
    let delegated_on_curve = keypair_from_bytes(&ON_CURVE_KEYPAIR);
    let delegated_account = match config.delegated_account {
        DelegatedAccountSetup::OffCurvePda => DELEGATED_PDA_ID,
        DelegatedAccountSetup::OnCurveKeypair => delegated_on_curve.pubkey(),
    };
    let delegation_rent_payer = match config.delegated_account {
        DelegatedAccountSetup::OffCurvePda => authority.pubkey(),
        DelegatedAccountSetup::OnCurveKeypair => payer.pubkey(),
    };

    add_system_account(&mut program_test, payer.pubkey(), LAMPORTS_PER_SOL);
    add_system_account(&mut program_test, authority.pubkey(), LAMPORTS_PER_SOL);

    add_delegated_account(&mut program_test, delegated_account);
    match config.delegated_account {
        DelegatedAccountSetup::OffCurvePda => {
            add_delegation_accounts_with_metadata(
                &mut program_test,
                delegated_account,
                authority.pubkey(),
                delegation_rent_payer,
                config.metadata_undelegatable,
                false,
            );
        }
        DelegatedAccountSetup::OnCurveKeypair => {
            add_delegation_accounts_with_metadata(
                &mut program_test,
                delegated_account,
                delegated_on_curve.pubkey(),
                delegation_rent_payer,
                config.metadata_undelegatable,
                true,
            );
        }
    }

    if config.with_commit_accounts {
        add_commit_accounts(
            &mut program_test,
            delegated_account,
            authority.pubkey(),
        );
    }
    if config.with_owner_program {
        add_owner_program(&mut program_test);
    }
    if config.with_fee_accounts {
        add_fee_accounts(&mut program_test, authority.pubkey());
    }
    if config.with_request_account {
        add_request_account(
            &mut program_test,
            delegated_account,
            delegation_rent_payer,
        );
    }

    let (banks, _, blockhash) = program_test.start().await;
    SetupContext {
        banks,
        payer,
        authority,
        delegated_on_curve,
        delegated_account,
        blockhash,
    }
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

fn add_delegation_accounts_with_metadata(
    program_test: &mut ProgramTest,
    delegated_account: Pubkey,
    authority: Pubkey,
    rent_payer: Pubkey,
    undelegatable: bool,
    on_curve: bool,
) {
    let delegation_record_data = if on_curve {
        get_delegation_record_on_curve_data(authority, Some(LAMPORTS_PER_SOL))
    } else {
        get_delegation_record_data(authority, None)
    };
    program_test.add_account(
        delegation_record_pda_from_delegated_account(&delegated_account),
        Account {
            lamports: Rent::default()
                .minimum_balance(delegation_record_data.len()),
            data: delegation_record_data,
            owner: dlp_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let delegation_metadata_data = if on_curve {
        get_delegation_metadata_data_on_curve(rent_payer, Some(undelegatable))
    } else {
        get_delegation_metadata_data(rent_payer, Some(undelegatable))
    };
    program_test.add_account(
        delegation_metadata_pda_from_delegated_account(&delegated_account),
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

fn add_commit_accounts(
    program_test: &mut ProgramTest,
    delegated_account: Pubkey,
    authority: Pubkey,
) {
    program_test.add_account(
        commit_state_pda_from_delegated_account(&delegated_account),
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
        commit_record_pda_from_delegated_account(&delegated_account),
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

fn add_request_account(
    program_test: &mut ProgramTest,
    delegated_account: Pubkey,
    rent_payer: Pubkey,
) {
    let request_data = create_undelegation_request_data(
        delegated_account,
        DELEGATED_PDA_OWNER_ID,
        rent_payer,
        1,
    );
    program_test.add_account(
        undelegation_request_pda_from_delegated_account(&delegated_account),
        Account {
            lamports: Rent::default().minimum_balance(request_data.len()),
            data: request_data,
            owner: dlp_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
}
