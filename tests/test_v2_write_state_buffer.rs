use dlp_api::v2::{
    instruction_builder::write_state_buffer, pda::state_buffer_pda,
    StateBuffer, WriteStateBufferArgs, STATE_BUFFER_MAX_ACCOUNT_GROWTH,
    STATE_BUFFER_MAX_TOTAL_LEN,
};
use solana_program::{hash::Hash, native_token::LAMPORTS_PER_SOL};
use solana_program_test::{
    BanksClient, BanksClientError, ProgramTestBanksClientExt,
};
use solana_sdk::{
    pubkey::Pubkey,
    rent::Rent,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use solana_system_interface::instruction as system_instruction;
use wheels::layout::Decodable;

mod fixtures;

use crate::fixtures::v2::{init_v2, setup_program_test_env, valid_args};

#[tokio::test]
async fn test_v2_write_state_buffer_unregistered_authority_one_chunk_finalizes()
{
    let mut env = setup_write_state_buffer_env().await;
    let commit_id = 7;
    let data = vec![1, 2, 3, 4];

    write_buffer(
        &mut env.banks,
        &env.payer,
        &env.writer,
        env.delegated.pubkey(),
        WriteStateBufferArgs {
            commit_id,
            total_len: data.len() as u32,
            offset: 0,
            chunk: data.clone(),
        },
    )
    .await
    .unwrap();

    let state_buffer = state_buffer_address(&env, commit_id);
    let state_buffer_account =
        get_state_buffer_account(&mut env.banks, state_buffer).await;
    let state = decode_state_buffer(&state_buffer_account.data);

    assert_eq!(state.discriminator(), StateBuffer::DISCRIMINATOR);
    assert_eq!(*state.authority(), env.writer.pubkey());
    assert_eq!(*state.account_pubkey(), env.delegated.pubkey());
    assert_eq!(state.commit_id(), commit_id);
    assert_eq!(state.total_len(), data.len() as u32);
    assert!(state.finalized());
    assert_eq!(*state.data_hash(), account_data_hash(&data));
    assert_eq!(state.payload().len(), data.len());
    assert_eq!(state.payload().capacity(), data.len());
    assert_eq!(state.payload().as_slice(), data.as_slice());
    assert_eq!(
        &state_buffer_account.data[StateBuffer::PAYLOAD_BYTES_OFFSET..],
        data.as_slice()
    );
}

#[tokio::test]
async fn test_v2_write_state_buffer_multiple_chunks_finalize() {
    let mut env = setup_write_state_buffer_env().await;
    let commit_id = 8;

    write_buffer(
        &mut env.banks,
        &env.payer,
        &env.writer,
        env.delegated.pubkey(),
        WriteStateBufferArgs {
            commit_id,
            total_len: 5,
            offset: 0,
            chunk: vec![1, 2],
        },
    )
    .await
    .unwrap();

    let state_buffer = state_buffer_address(&env, commit_id);
    let state_buffer_account =
        get_state_buffer_account(&mut env.banks, state_buffer).await;
    let state = decode_state_buffer(&state_buffer_account.data);
    assert_rent_exempt(&state_buffer_account);
    assert_payload_prefix_and_zero_suffix(
        &state_buffer_account.data,
        &state,
        &[1, 2],
        5,
    );
    assert!(!state.finalized());
    assert_eq!(*state.data_hash(), [0; 32]);

    write_buffer(
        &mut env.banks,
        &env.payer,
        &env.writer,
        env.delegated.pubkey(),
        WriteStateBufferArgs {
            commit_id,
            total_len: 5,
            offset: 2,
            chunk: vec![3, 4, 5],
        },
    )
    .await
    .unwrap();

    let state_buffer = state_buffer_address(&env, commit_id);
    let state_buffer_account =
        get_state_buffer_account(&mut env.banks, state_buffer).await;
    let state = decode_state_buffer(&state_buffer_account.data);
    assert_eq!(state.payload().len(), 5);
    assert_eq!(state.payload().capacity(), 5);
    assert_eq!(state.payload().as_slice(), &[1, 2, 3, 4, 5]);
    assert!(state.finalized());
    assert_eq!(*state.data_hash(), account_data_hash(&[1, 2, 3, 4, 5]));
    assert_eq!(
        &state_buffer_account.data[StateBuffer::PAYLOAD_BYTES_OFFSET..],
        &[1, 2, 3, 4, 5]
    );
}

#[tokio::test]
async fn test_v2_write_state_buffer_grows_payload_span_past_initial_capacity() {
    let mut env = setup_write_state_buffer_env().await;
    let commit_id = 15;
    let initial_payload_capacity = initial_payload_capacity();
    let total_len = initial_payload_capacity + 11;
    let first_write_len = initial_payload_capacity - 3;
    let mut expected = Vec::with_capacity(total_len);
    let mut offset = 0;

    while offset < first_write_len {
        let chunk_len = (first_write_len - offset).min(512);
        let chunk = chunk_for_offset(offset, chunk_len);
        expected.extend_from_slice(&chunk);

        write_buffer(
            &mut env.banks,
            &env.payer,
            &env.writer,
            env.delegated.pubkey(),
            WriteStateBufferArgs {
                commit_id,
                total_len: total_len as u32,
                offset: offset as u32,
                chunk,
            },
        )
        .await
        .unwrap();

        offset += chunk_len;
    }

    let state_buffer = state_buffer_address(&env, commit_id);
    let state_buffer_account =
        get_state_buffer_account(&mut env.banks, state_buffer).await;
    let state = decode_state_buffer(&state_buffer_account.data);
    assert_rent_exempt(&state_buffer_account);
    assert_payload_prefix_and_zero_suffix(
        &state_buffer_account.data,
        &state,
        &expected,
        initial_payload_capacity,
    );
    assert!(!state.finalized());

    let crossing_chunk = vec![7, 8, 9, 10, 11];
    write_buffer(
        &mut env.banks,
        &env.payer,
        &env.writer,
        env.delegated.pubkey(),
        WriteStateBufferArgs {
            commit_id,
            total_len: total_len as u32,
            offset: first_write_len as u32,
            chunk: crossing_chunk.clone(),
        },
    )
    .await
    .unwrap();
    expected.extend_from_slice(&crossing_chunk);

    let state_buffer = state_buffer_address(&env, commit_id);
    let state_buffer_account =
        get_state_buffer_account(&mut env.banks, state_buffer).await;
    let state = decode_state_buffer(&state_buffer_account.data);
    assert_payload_prefix_and_zero_suffix(
        &state_buffer_account.data,
        &state,
        &expected,
        total_len,
    );
    assert!(!state.finalized());
}

#[tokio::test]
async fn test_v2_write_state_buffer_duplicate_retry() {
    let mut env = setup_write_state_buffer_env().await;
    let commit_id = 9;
    let first_chunk = vec![1, 2];

    write_buffer(
        &mut env.banks,
        &env.payer,
        &env.writer,
        env.delegated.pubkey(),
        WriteStateBufferArgs {
            commit_id,
            total_len: 4,
            offset: 0,
            chunk: first_chunk.clone(),
        },
    )
    .await
    .unwrap();

    write_buffer(
        &mut env.banks,
        &env.payer,
        &env.writer,
        env.delegated.pubkey(),
        WriteStateBufferArgs {
            commit_id,
            total_len: 4,
            offset: 0,
            chunk: first_chunk,
        },
    )
    .await
    .unwrap();

    let state_buffer = state_buffer_address(&env, commit_id);
    let state_buffer_account =
        get_state_buffer_account(&mut env.banks, state_buffer).await;
    let state = decode_state_buffer(&state_buffer_account.data);
    assert_payload_prefix_and_zero_suffix(
        &state_buffer_account.data,
        &state,
        &[1, 2],
        4,
    );
    assert!(!state.finalized());
}

#[tokio::test]
async fn test_v2_write_state_buffer_rejects_mismatched_duplicate_retry() {
    let mut env = setup_write_state_buffer_env().await;
    let commit_id = 10;

    write_buffer(
        &mut env.banks,
        &env.payer,
        &env.writer,
        env.delegated.pubkey(),
        WriteStateBufferArgs {
            commit_id,
            total_len: 4,
            offset: 0,
            chunk: vec![1, 2],
        },
    )
    .await
    .unwrap();

    let result = write_buffer(
        &mut env.banks,
        &env.payer,
        &env.writer,
        env.delegated.pubkey(),
        WriteStateBufferArgs {
            commit_id,
            total_len: 4,
            offset: 0,
            chunk: vec![1, 9],
        },
    )
    .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_v2_write_state_buffer_rejects_wrong_offset() {
    let mut env = setup_write_state_buffer_env().await;

    let result = write_buffer(
        &mut env.banks,
        &env.payer,
        &env.writer,
        env.delegated.pubkey(),
        WriteStateBufferArgs {
            commit_id: 11,
            total_len: 4,
            offset: 1,
            chunk: vec![1, 2],
        },
    )
    .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_v2_write_state_buffer_rejects_oversized_total_len() {
    let mut env = setup_write_state_buffer_env().await;

    let result = write_buffer(
        &mut env.banks,
        &env.payer,
        &env.writer,
        env.delegated.pubkey(),
        WriteStateBufferArgs {
            commit_id: 12,
            total_len: STATE_BUFFER_MAX_TOTAL_LEN + 1,
            offset: 0,
            chunk: vec![1],
        },
    )
    .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_v2_write_state_buffer_rejects_post_finalize_mutation() {
    let mut env = setup_write_state_buffer_env().await;
    let commit_id = 13;
    let data = vec![1, 2, 3];

    write_buffer(
        &mut env.banks,
        &env.payer,
        &env.writer,
        env.delegated.pubkey(),
        WriteStateBufferArgs {
            commit_id,
            total_len: data.len() as u32,
            offset: 0,
            chunk: data.clone(),
        },
    )
    .await
    .unwrap();

    write_buffer(
        &mut env.banks,
        &env.payer,
        &env.writer,
        env.delegated.pubkey(),
        WriteStateBufferArgs {
            commit_id,
            total_len: data.len() as u32,
            offset: 0,
            chunk: data,
        },
    )
    .await
    .unwrap();

    let result = write_buffer(
        &mut env.banks,
        &env.payer,
        &env.writer,
        env.delegated.pubkey(),
        WriteStateBufferArgs {
            commit_id,
            total_len: 3,
            offset: 3,
            chunk: vec![4],
        },
    )
    .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_v2_write_state_buffer_rejects_wrong_authority_for_buffer() {
    let mut env = setup_write_state_buffer_env().await;
    let commit_id = 14;
    let wrong_authority = Keypair::new();
    fund_account(&mut env.banks, &env.payer, &wrong_authority.pubkey()).await;

    let mut ix = write_state_buffer(
        env.payer.pubkey(),
        wrong_authority.pubkey(),
        env.delegated.pubkey(),
        WriteStateBufferArgs {
            commit_id,
            total_len: 2,
            offset: 0,
            chunk: vec![1, 2],
        },
    );
    ix.accounts[2].pubkey = state_buffer_pda(
        &env.delegated.pubkey(),
        commit_id,
        &env.writer.pubkey(),
    );

    let blockhash = fresh_blockhash(&mut env.banks).await;
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&env.payer.pubkey()),
        &[&env.payer, &wrong_authority],
        blockhash,
    );

    assert!(env.banks.process_transaction(tx).await.is_err());
}

struct WriteStateBufferEnv {
    banks: BanksClient,
    payer: Keypair,
    writer: Keypair,
    delegated: Keypair,
}

async fn setup_write_state_buffer_env() -> WriteStateBufferEnv {
    let (mut banks, payer, authority, blockhash) =
        setup_program_test_env().await;
    let writer = Keypair::new();
    let delegated = Keypair::new();
    let config_args = valid_args();

    init_v2(&banks, &payer, &authority, blockhash, config_args).await;
    fund_account(&mut banks, &payer, &writer.pubkey()).await;
    create_delegated_account(&mut banks, &payer, &delegated).await;

    WriteStateBufferEnv {
        banks,
        payer,
        writer,
        delegated,
    }
}

async fn create_delegated_account(
    banks: &mut BanksClient,
    payer: &Keypair,
    delegated: &Keypair,
) {
    let lamports = Rent::default().minimum_balance(8);
    let ix = system_instruction::create_account(
        &payer.pubkey(),
        &delegated.pubkey(),
        lamports,
        8,
        &dlp_api::ID,
    );
    let blockhash = fresh_blockhash(banks).await;
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer, delegated],
        blockhash,
    );

    banks.process_transaction(tx).await.unwrap();
}

async fn fund_account(
    banks: &mut BanksClient,
    payer: &Keypair,
    account: &Pubkey,
) {
    let ix = system_instruction::transfer(
        &payer.pubkey(),
        account,
        LAMPORTS_PER_SOL,
    );
    let blockhash = fresh_blockhash(banks).await;
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer],
        blockhash,
    );

    banks.process_transaction(tx).await.unwrap();
}

async fn write_buffer(
    banks: &mut BanksClient,
    payer: &Keypair,
    authority: &Keypair,
    account: Pubkey,
    args: WriteStateBufferArgs,
) -> Result<(), BanksClientError> {
    let ix =
        write_state_buffer(payer.pubkey(), authority.pubkey(), account, args);
    let blockhash = fresh_blockhash(banks).await;
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer, authority],
        blockhash,
    );

    banks.process_transaction(tx).await
}

async fn get_state_buffer_account(
    banks: &mut BanksClient,
    state_buffer: Pubkey,
) -> solana_sdk::account::Account {
    banks.get_account(state_buffer).await.unwrap().unwrap()
}

fn state_buffer_address(env: &WriteStateBufferEnv, commit_id: u64) -> Pubkey {
    state_buffer_pda(&env.delegated.pubkey(), commit_id, &env.writer.pubkey())
}

fn decode_state_buffer(data: &[u8]) -> dlp_api::v2::StateBufferView<'_> {
    <StateBuffer as Decodable>::decode(data).unwrap()
}

fn initial_payload_capacity() -> usize {
    STATE_BUFFER_MAX_ACCOUNT_GROWTH - StateBuffer::PAYLOAD_BYTES_OFFSET
}

fn assert_payload_prefix_and_zero_suffix(
    account_data: &[u8],
    state: &dlp_api::v2::StateBufferView<'_>,
    prefix: &[u8],
    payload_capacity: usize,
) {
    let payload = state.payload();
    assert_eq!(payload.len(), prefix.len());
    assert_eq!(payload.capacity(), payload_capacity);
    assert_eq!(payload.as_slice(), prefix);

    let payload_start = StateBuffer::PAYLOAD_BYTES_OFFSET;
    let written_end = payload_start + prefix.len();
    let capacity_end = payload_start + payload_capacity;
    assert_eq!(&account_data[payload_start..written_end], prefix);
    assert!(account_data[written_end..capacity_end]
        .iter()
        .all(|byte| *byte == 0));
}

fn chunk_for_offset(offset: usize, len: usize) -> Vec<u8> {
    (offset..offset + len)
        .map(|value| (value % u8::MAX as usize) as u8)
        .collect()
}

fn assert_rent_exempt(account: &solana_sdk::account::Account) {
    assert!(
        account.lamports >= Rent::default().minimum_balance(account.data.len())
    );
}

async fn fresh_blockhash(banks: &mut BanksClient) -> Hash {
    let latest_blockhash = banks.get_latest_blockhash().await.unwrap();
    banks
        .get_new_latest_blockhash(&latest_blockhash)
        .await
        .unwrap()
}

fn account_data_hash(data: &[u8]) -> [u8; 32] {
    solana_sha256_hasher::hashv(&[b"magicblock.account_data.v1", data])
        .to_bytes()
}
