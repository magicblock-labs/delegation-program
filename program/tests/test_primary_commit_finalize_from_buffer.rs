use delegation_program::solana_program;
use delegation_program_interface as dpi;
use dpi::{
    diff::compute_diff,
    instruction_builder::{
        commit_finalize_from_buffer, prepare_commit_finalize_instructions,
        write_state_buffer,
    },
    pda::{delegation_cluster_pda, delegation_state_pda, state_buffer_pda},
    CommitFinalizeFromBufferArgs, CommitPayloadKind, DelegationCluster,
    DelegationState, StateBuffer, UndelegationRequester, WriteStateBufferArgs,
};
use solana_program::{
    native_token::LAMPORTS_PER_SOL, pubkey::Pubkey, rent::Rent,
};
use solana_program_test::{BanksClient, ProgramTest};
use solana_sdk::{
    account::Account,
    instruction::Instruction,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use solana_sdk_ids::system_program;
use wheels::layout::{Decodable, Encodable};

const INITIAL_LAMPORTS: u64 = LAMPORTS_PER_SOL;

struct CommitFinalizeFromBufferTestContext {
    banks: BanksClient,
    payer: Keypair,
    current_validator: Keypair,
    delegated_account: Pubkey,
    delegation_state: Pubkey,
    delegation_cluster: Pubkey,
    original_owner: Pubkey,
}

struct PreloadedBuffer {
    commit_id: u64,
    payload: Vec<u8>,
    data_hash: [u8; 32],
}

#[tokio::test]
async fn test_commit_finalize_from_buffer_applies_large_chunked_payload() {
    let initial_data = b"initial data";
    let context = setup_test(initial_data, None).await;
    let target_len = 2 * pinocchio::account::MAX_PERMITTED_DATA_INCREASE + 17;
    let payload = (0..target_len)
        .map(|index| (index % u8::MAX as usize) as u8)
        .collect::<Vec<_>>();

    upload_payload(&context, 1, &payload).await;

    let mut instructions = prepare_commit_finalize_instructions(
        context.current_validator.pubkey(),
        context.delegation_cluster,
        context.delegated_account,
        1,
        initial_data.len(),
        target_len as u32,
    )
    .unwrap();
    assert_eq!(instructions.len(), 3);
    instructions.push(finalize_instruction(
        &context,
        1,
        CommitPayloadKind::FullData,
    ));
    submit_instructions(&context, instructions).await.unwrap();

    let delegated_account = context
        .banks
        .get_account(context.delegated_account)
        .await
        .unwrap()
        .unwrap();
    let state_account = context
        .banks
        .get_account(context.delegation_state)
        .await
        .unwrap()
        .unwrap();
    let state = DelegationState::decode(&state_account.data).unwrap();
    let buffer_account = context
        .banks
        .get_account(state_buffer_pda(
            &context.delegated_account,
            1,
            &context.current_validator.pubkey(),
        ))
        .await
        .unwrap()
        .unwrap();

    assert_eq!(delegated_account.data, payload);
    assert_eq!(state.last_commit_id(), 1);
    assert_eq!(state.lamports(), INITIAL_LAMPORTS);
    assert!(StateBuffer::decode(&buffer_account.data)
        .unwrap()
        .finalized());
}

#[tokio::test]
async fn test_commit_finalize_from_buffer_applies_aligned_diff() {
    let initial_data = b"the original account state";
    let context = setup_test(initial_data, None).await;
    let changed_data = b"the changed account state!";
    let diff = compute_diff(initial_data, changed_data).to_vec();

    upload_payload(&context, 1, &diff).await;
    submit_instructions(
        &context,
        vec![finalize_instruction(&context, 1, CommitPayloadKind::Diff)],
    )
    .await
    .unwrap();

    let delegated_account = context
        .banks
        .get_account(context.delegated_account)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(delegated_account.data, changed_data);
}

#[tokio::test]
async fn test_commit_finalize_from_buffer_rejects_invalid_finalized_hash() {
    let payload = b"buffered state".to_vec();
    let context = setup_test(
        b"initial data",
        Some(PreloadedBuffer {
            commit_id: 1,
            payload,
            data_hash: [7; 32],
        }),
    )
    .await;

    assert!(submit_instructions(
        &context,
        vec![finalize_instruction(
            &context,
            1,
            CommitPayloadKind::FullData,
        )],
    )
    .await
    .is_err());

    let state_account = context
        .banks
        .get_account(context.delegation_state)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        DelegationState::decode(&state_account.data)
            .unwrap()
            .last_commit_id(),
        0
    );
}

fn finalize_instruction(
    context: &CommitFinalizeFromBufferTestContext,
    commit_id: u64,
    payload_kind: CommitPayloadKind,
) -> Instruction {
    commit_finalize_from_buffer(
        context.current_validator.pubkey(),
        context.delegation_cluster,
        context.delegated_account,
        CommitFinalizeFromBufferArgs {
            commit_id,
            lamports: INITIAL_LAMPORTS,
            owner: context.original_owner,
            allow_undelegation: false,
            payload_kind: payload_kind.value(),
        },
    )
    .unwrap()
}

async fn upload_payload(
    context: &CommitFinalizeFromBufferTestContext,
    commit_id: u64,
    payload: &[u8],
) {
    let mut offset = 0;
    while offset < payload.len() {
        let end = (offset + 512).min(payload.len());
        let instruction = write_state_buffer(
            context.payer.pubkey(),
            context.current_validator.pubkey(),
            context.delegated_account,
            WriteStateBufferArgs {
                commit_id,
                total_len: payload.len() as u32,
                offset: offset as u32,
                chunk: payload[offset..end].to_vec(),
            },
        );
        let tx = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&context.payer.pubkey()),
            &[&context.payer, &context.current_validator],
            context.banks.get_latest_blockhash().await.unwrap(),
        );
        context.banks.process_transaction(tx).await.unwrap();
        offset = end;
    }
}

async fn submit_instructions(
    context: &CommitFinalizeFromBufferTestContext,
    instructions: Vec<Instruction>,
) -> Result<(), solana_program_test::BanksClientError> {
    let tx = Transaction::new_signed_with_payer(
        &instructions,
        Some(&context.payer.pubkey()),
        &[&context.payer, &context.current_validator],
        context.banks.get_latest_blockhash().await.unwrap(),
    );
    context.banks.process_transaction(tx).await
}

async fn setup_test(
    initial_data: &[u8],
    preloaded_buffer: Option<PreloadedBuffer>,
) -> CommitFinalizeFromBufferTestContext {
    let mut program_test = ProgramTest::default();
    program_test.prefer_bpf(true);
    program_test.add_program("delegation_program", dpi::ID, None);

    let current_validator = Keypair::new();
    let delegated_account = Pubkey::new_unique();
    let original_owner = Pubkey::new_unique();
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
        delegated_account,
        Account {
            lamports: INITIAL_LAMPORTS,
            data: initial_data.to_vec(),
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
        paused: false,
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
        rent_payer: Pubkey::new_unique(),
        delegation_slot: 0,
        lamports: INITIAL_LAMPORTS,
        last_commit_id: 0,
        undelegation_requester: UndelegationRequester::None.value(),
        delegated_account_bump: 0,
        seed_count: 0,
        encoded_seeds: vec![],
    }
    .encode()
    .unwrap();
    program_test.add_account(
        delegation_state,
        Account {
            lamports: Rent::default().minimum_balance(state_data.len()),
            data: state_data,
            owner: dpi::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    if let Some(preloaded) = preloaded_buffer {
        let state_buffer = state_buffer_pda(
            &delegated_account,
            preloaded.commit_id,
            &current_validator.pubkey(),
        );
        let (_, bump) = Pubkey::find_program_address(
            &[
                dpi::pda::STATE_BUFFER_SEED,
                delegated_account.as_ref(),
                &preloaded.commit_id.to_le_bytes(),
                current_validator.pubkey().as_ref(),
            ],
            &dpi::id(),
        );
        let buffer_data = StateBuffer {
            discriminator: StateBuffer::DISCRIMINATOR,
            bump,
            authority: current_validator.pubkey(),
            account_pubkey: delegated_account,
            commit_id: preloaded.commit_id,
            data_hash: preloaded.data_hash,
            total_len: preloaded.payload.len() as u32,
            finalized: true,
            payload_alignment_padding: [0; 2],
            payload: preloaded.payload,
        }
        .encode()
        .unwrap();
        program_test.add_account(
            state_buffer,
            Account {
                lamports: Rent::default().minimum_balance(buffer_data.len()),
                data: buffer_data,
                owner: dpi::id(),
                executable: false,
                rent_epoch: 0,
            },
        );
    }

    let (banks, payer, _) = program_test.start().await;
    CommitFinalizeFromBufferTestContext {
        banks,
        payer,
        current_validator,
        delegated_account,
        delegation_state,
        delegation_cluster,
        original_owner,
    }
}
