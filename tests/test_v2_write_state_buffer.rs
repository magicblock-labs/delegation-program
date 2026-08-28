use dlp_api::v2::{
    instruction_builder::{register_operator, write_state_buffer},
    pda::state_buffer_pda,
    DlpV2Instruction, RegisterOperatorArgs, StateBuffer, WriteStateBufferArgs,
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
use wheels::layout::{Decodable, Encodable};

mod fixtures;

use crate::fixtures::v2::{init_v2, setup_program_test_env, valid_args};

#[test]
fn test_v2_write_state_buffer_instruction_data_uses_one_byte_tag() {
    let args = WriteStateBufferArgs {
        commit_id: 7,
        total_len: 3,
        offset: 0,
        chunk: vec![1, 2, 3],
    };
    let ix = write_state_buffer(
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        args.clone(),
    );
    let encoded_args = args.encode().unwrap();

    assert_eq!(ix.data[0], DlpV2Instruction::WriteStateBuffer as u8);
    assert_eq!(ix.data.len(), 1 + encoded_args.len());
    assert_eq!(&ix.data[1..], encoded_args.as_slice());

    let decoded =
        <WriteStateBufferArgs as Decodable>::decode(&ix.data[1..]).unwrap();
    assert_eq!(decoded.commit_id(), args.commit_id);
    assert_eq!(decoded.total_len(), args.total_len);
    assert_eq!(decoded.offset(), args.offset);
    assert_eq!(decoded.chunk(), args.chunk.as_slice());
}

#[tokio::test]
async fn test_v2_write_state_buffer_one_chunk_finalizes() {
    let mut env = setup_write_state_buffer_env().await;
    let commit_id = 7;
    let data = vec![1, 2, 3, 4];

    write_buffer(
        &mut env.banks,
        &env.payer,
        &env.operator,
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
    assert_eq!(*state.authority(), env.operator.pubkey());
    assert_eq!(*state.account_pubkey(), env.delegated.pubkey());
    assert_eq!(state.commit_id(), commit_id);
    assert_eq!(state.total_len(), data.len() as u32);
    assert_eq!(state.written_len(), data.len() as u32);
    assert!(state.finalized());
    assert_eq!(*state.data_hash(), account_data_hash(&data));
    assert_eq!(&state_buffer_account.data[StateBuffer::DATA_LEN..], &data);
}

#[tokio::test]
async fn test_v2_write_state_buffer_multiple_chunks_finalize() {
    let mut env = setup_write_state_buffer_env().await;
    let commit_id = 8;

    write_buffer(
        &mut env.banks,
        &env.payer,
        &env.operator,
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
    assert_eq!(state.written_len(), 2);
    assert!(!state.finalized());
    assert_eq!(*state.data_hash(), [0; 32]);

    write_buffer(
        &mut env.banks,
        &env.payer,
        &env.operator,
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
    assert_eq!(state.written_len(), 5);
    assert!(state.finalized());
    assert_eq!(*state.data_hash(), account_data_hash(&[1, 2, 3, 4, 5]));
    assert_eq!(
        &state_buffer_account.data[StateBuffer::DATA_LEN..],
        &[1, 2, 3, 4, 5]
    );
}

#[tokio::test]
async fn test_v2_write_state_buffer_duplicate_retry() {
    let mut env = setup_write_state_buffer_env().await;
    let commit_id = 9;
    let first_chunk = vec![1, 2];

    write_buffer(
        &mut env.banks,
        &env.payer,
        &env.operator,
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
        &env.operator,
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
    assert_eq!(state.written_len(), 2);
    assert!(!state.finalized());
}

#[tokio::test]
async fn test_v2_write_state_buffer_rejects_mismatched_duplicate_retry() {
    let mut env = setup_write_state_buffer_env().await;
    let commit_id = 10;

    write_buffer(
        &mut env.banks,
        &env.payer,
        &env.operator,
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
        &env.operator,
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
        &env.operator,
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
        &env.operator,
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
        &env.operator,
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
        &env.operator,
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
        &env.operator,
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
async fn test_v2_write_state_buffer_rejects_wrong_operator_for_buffer() {
    let mut env = setup_write_state_buffer_env().await;
    let commit_id = 14;
    let wrong_operator = Keypair::new();
    fund_account(&mut env.banks, &env.payer, &wrong_operator.pubkey()).await;
    register_test_operator(
        &mut env.banks,
        &env.payer,
        &env.authority,
        &wrong_operator,
        env.config_args.min_operator_bond,
    )
    .await;

    let mut ix = write_state_buffer(
        env.payer.pubkey(),
        wrong_operator.pubkey(),
        env.delegated.pubkey(),
        WriteStateBufferArgs {
            commit_id,
            total_len: 2,
            offset: 0,
            chunk: vec![1, 2],
        },
    );
    ix.accounts[3].pubkey = state_buffer_pda(
        &env.delegated.pubkey(),
        commit_id,
        &env.operator.pubkey(),
    );

    let blockhash = fresh_blockhash(&mut env.banks).await;
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&env.payer.pubkey()),
        &[&env.payer, &wrong_operator],
        blockhash,
    );

    assert!(env.banks.process_transaction(tx).await.is_err());
}

struct WriteStateBufferEnv {
    banks: BanksClient,
    payer: Keypair,
    authority: Keypair,
    operator: Keypair,
    delegated: Keypair,
    config_args: dlp_api::v2::InitProtocolConfigArgs,
}

async fn setup_write_state_buffer_env() -> WriteStateBufferEnv {
    let (mut banks, payer, authority, blockhash) =
        setup_program_test_env().await;
    let operator = Keypair::new();
    let delegated = Keypair::new();
    let config_args = valid_args();

    init_v2(&banks, &payer, &authority, blockhash, config_args.clone()).await;
    fund_account(&mut banks, &payer, &operator.pubkey()).await;
    register_test_operator(
        &mut banks,
        &payer,
        &authority,
        &operator,
        config_args.min_operator_bond,
    )
    .await;
    create_delegated_account(&mut banks, &payer, &delegated).await;

    WriteStateBufferEnv {
        banks,
        payer,
        authority,
        operator,
        delegated,
        config_args,
    }
}

async fn register_test_operator(
    banks: &mut BanksClient,
    payer: &Keypair,
    authority: &Keypair,
    operator: &Keypair,
    amount_lamports: u64,
) {
    let ix = register_operator(
        operator.pubkey(),
        authority.pubkey(),
        RegisterOperatorArgs { amount_lamports },
    );
    let blockhash = fresh_blockhash(banks).await;
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer, operator, authority],
        blockhash,
    );

    banks.process_transaction(tx).await.unwrap();
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
    operator: &Keypair,
    account: Pubkey,
    args: WriteStateBufferArgs,
) -> Result<(), BanksClientError> {
    let ix =
        write_state_buffer(payer.pubkey(), operator.pubkey(), account, args);
    let blockhash = fresh_blockhash(banks).await;
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer, operator],
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
    state_buffer_pda(&env.delegated.pubkey(), commit_id, &env.operator.pubkey())
}

fn decode_state_buffer(data: &[u8]) -> dlp_api::v2::StateBufferView<'_> {
    <StateBuffer as Decodable>::decode(&data[..StateBuffer::DATA_LEN]).unwrap()
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
