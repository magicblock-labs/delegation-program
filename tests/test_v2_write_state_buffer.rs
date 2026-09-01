use dlp_api::v2::{
    instruction_builder::write_state_buffer, pda::state_buffer_pda,
    StateBuffer, WriteStateBufferArgs,
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

use crate::fixtures::v2::{
    initialize_protocol_config, setup_program_test_env,
    valid_protocol_config_args,
};

#[tokio::test]
async fn test_write_state_buffer_unregistered_authority_one_chunk_finalizes() {
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

    let state_buffer = state_buffer_pda(
        &env.delegated.pubkey(),
        commit_id,
        &env.writer.pubkey(),
    );
    let state_buffer_account =
        env.banks.get_account(state_buffer).await.unwrap().unwrap();
    let state = StateBuffer::decode(&state_buffer_account.data).unwrap();

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
async fn test_write_state_buffer_multiple_chunks_finalize() {
    let mut env = setup_write_state_buffer_env().await;
    let commit_id = 8;
    let total_len = 5;
    let state_buffer = state_buffer_pda(
        &env.delegated.pubkey(),
        commit_id,
        &env.writer.pubkey(),
    );

    {
        write_buffer(
            &mut env.banks,
            &env.payer,
            &env.writer,
            env.delegated.pubkey(),
            WriteStateBufferArgs {
                commit_id,
                total_len,
                offset: 0,
                chunk: vec![1, 2],
            },
        )
        .await
        .unwrap();

        let state_buffer_account =
            env.banks.get_account(state_buffer).await.unwrap().unwrap();
        let state = StateBuffer::decode(&state_buffer_account.data).unwrap();
        assert!(
            state_buffer_account.lamports
                >= Rent::default()
                    .minimum_balance(state_buffer_account.data.len())
        );
        let payload = state.payload();
        assert_eq!(payload.len(), 2);
        assert_eq!(payload.capacity(), total_len as usize);
        assert_eq!(payload.as_slice(), &[1, 2]);

        let payload_start = StateBuffer::PAYLOAD_BYTES_OFFSET;
        let written_end = payload_start + 2;
        let capacity_end = payload_start + total_len as usize;
        assert_eq!(
            &state_buffer_account.data[payload_start..written_end],
            &[1, 2]
        );
        assert!(state_buffer_account.data[written_end..capacity_end]
            .iter()
            .all(|byte| *byte == 0));
        assert!(!state.finalized());
        assert_eq!(*state.data_hash(), [0; 32]);
    }

    {
        write_buffer(
            &mut env.banks,
            &env.payer,
            &env.writer,
            env.delegated.pubkey(),
            WriteStateBufferArgs {
                commit_id,
                total_len,
                offset: 2,
                chunk: vec![3, 4, 5],
            },
        )
        .await
        .unwrap();

        let state_buffer_account =
            env.banks.get_account(state_buffer).await.unwrap().unwrap();
        let state = StateBuffer::decode(&state_buffer_account.data).unwrap();
        let payload = state.payload();
        assert_eq!(payload.len(), total_len as usize);
        assert_eq!(payload.capacity(), total_len as usize);
        assert_eq!(payload.as_slice(), &[1, 2, 3, 4, 5]);

        let payload_start = StateBuffer::PAYLOAD_BYTES_OFFSET;
        let payload_end = payload_start + total_len as usize;
        assert_eq!(
            &state_buffer_account.data[payload_start..payload_end],
            &[1, 2, 3, 4, 5]
        );
        assert!(state.finalized());
        assert_eq!(*state.data_hash(), account_data_hash(&[1, 2, 3, 4, 5]));
    }
}

#[tokio::test]
async fn test_write_state_buffer_grows_payload_span_past_initial_capacity() {
    let mut env = setup_write_state_buffer_env().await;
    let commit_id = 15;
    let state_buffer = state_buffer_pda(
        &env.delegated.pubkey(),
        commit_id,
        &env.writer.pubkey(),
    );
    let initial_payload_capacity = StateBuffer::MAX_INITIAL_PAYLOAD_LEN;
    let total_len = initial_payload_capacity + 11;
    let final_data_len =
        StateBuffer::data_len_from_payload_capacity(total_len).unwrap();
    let first_write_len = initial_payload_capacity - 3;
    let mut expected = Vec::with_capacity(total_len);
    let mut offset = 0;

    println!("total_len: {total_len}, first_write_len: {first_write_len}");

    while offset < first_write_len {
        let chunk_len = (first_write_len - offset).min(512);
        let chunk = (offset..offset + chunk_len)
            .map(|value| (value % u8::MAX as usize) as u8)
            .collect::<Vec<_>>();
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

    assert_eq!(offset, first_write_len);

    let state_buffer_account =
        env.banks.get_account(state_buffer).await.unwrap().unwrap();
    let state = StateBuffer::decode(&state_buffer_account.data).unwrap();
    assert!(
        state_buffer_account.lamports
            >= Rent::default().minimum_balance(final_data_len)
    );
    let payload = state.payload();
    assert_eq!(payload.len(), expected.len());
    assert_eq!(payload.capacity(), initial_payload_capacity);
    assert_eq!(payload.as_slice(), expected.as_slice());

    let payload_start = StateBuffer::PAYLOAD_BYTES_OFFSET;
    let written_end = payload_start + expected.len();
    let capacity_end = payload_start + initial_payload_capacity;
    assert_eq!(
        &state_buffer_account.data[payload_start..written_end],
        expected.as_slice()
    );
    assert!(state_buffer_account.data[written_end..capacity_end]
        .iter()
        .all(|byte| *byte == 0));
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

    let state_buffer_account =
        env.banks.get_account(state_buffer).await.unwrap().unwrap();
    let state = StateBuffer::decode(&state_buffer_account.data).unwrap();
    let payload = state.payload();
    assert_eq!(payload.len(), expected.len());
    assert_eq!(payload.capacity(), total_len);
    assert_eq!(payload.as_slice(), expected.as_slice());

    let payload_start = StateBuffer::PAYLOAD_BYTES_OFFSET;
    let written_end = payload_start + expected.len();
    let capacity_end = payload_start + total_len;
    assert_eq!(
        &state_buffer_account.data[payload_start..written_end],
        expected.as_slice()
    );
    assert!(state_buffer_account.data[written_end..capacity_end]
        .iter()
        .all(|byte| *byte == 0));
    assert!(!state.finalized());
}

#[tokio::test]
async fn test_write_state_buffer_duplicate_retry() {
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

    let state_buffer = state_buffer_pda(
        &env.delegated.pubkey(),
        commit_id,
        &env.writer.pubkey(),
    );
    let state_buffer_account =
        env.banks.get_account(state_buffer).await.unwrap().unwrap();
    let state = StateBuffer::decode(&state_buffer_account.data).unwrap();
    let payload = state.payload();
    assert_eq!(payload.len(), 2);
    assert_eq!(payload.capacity(), 4);
    assert_eq!(payload.as_slice(), &[1, 2]);

    let payload_start = StateBuffer::PAYLOAD_BYTES_OFFSET;
    let written_end = payload_start + 2;
    let capacity_end = payload_start + 4;
    assert_eq!(
        &state_buffer_account.data[payload_start..written_end],
        &[1, 2]
    );
    assert!(state_buffer_account.data[written_end..capacity_end]
        .iter()
        .all(|byte| *byte == 0));
    assert!(!state.finalized());
}

#[tokio::test]
async fn test_write_state_buffer_rejects_mismatched_duplicate_retry() {
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
async fn test_write_state_buffer_rejects_wrong_offset() {
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
async fn test_write_state_buffer_rejects_oversized_total_len() {
    let mut env = setup_write_state_buffer_env().await;

    let result = write_buffer(
        &mut env.banks,
        &env.payer,
        &env.writer,
        env.delegated.pubkey(),
        WriteStateBufferArgs {
            commit_id: 12,
            total_len: StateBuffer::MAX_TOTAL_PAYLOAD_LEN + 1,
            offset: 0,
            chunk: vec![1],
        },
    )
    .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_write_state_buffer_rejects_post_finalize_mutation() {
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
async fn test_write_state_buffer_rejects_wrong_authority_for_buffer() {
    let mut env = setup_write_state_buffer_env().await;
    let commit_id = 14;
    let wrong_authority = Keypair::new();
    fund_account(&mut env.banks, &env.payer, &wrong_authority.pubkey()).await;

    let mut ix = write_state_buffer(
        env.payer.pubkey(),
        env.writer.pubkey(),
        env.delegated.pubkey(),
        WriteStateBufferArgs {
            commit_id,
            total_len: 2,
            offset: 0,
            chunk: vec![1, 2],
        },
    );

    // use wrong_authority as authority
    ix.accounts[1].pubkey = wrong_authority.pubkey();

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
    let config_args = valid_protocol_config_args();

    initialize_protocol_config(
        &banks,
        &payer,
        &authority,
        blockhash,
        config_args,
    )
    .await;
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
