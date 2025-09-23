use borsh::{BorshDeserialize, BorshSerialize};
use dlp::args::DelegateCompressedArgs;
use dlp::state::DelegatedCompressedAccount;
use light_program_test::{AddressWithTree, Indexer, LightProgramTest, ProgramTestConfig, Rpc};
use light_sdk::address::v1::derive_address;
use light_sdk::instruction::account_meta::CompressedAccountMeta;
use light_sdk::instruction::{PackedAccounts, SystemAccountMetaConfig};
use solana_program::pubkey::Pubkey;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::signature::{Keypair, Signer};

use crate::fixtures::{
    DELEGATED_PDA_OWNER_ID, DELEGATED_PDA_SEED,
    EXTERNAL_DELEGATE_COMPRESSED_INSTRUCTION_DISCRIMINATOR, TEST_AUTHORITY,
};

mod fixtures;

#[tokio::test]
async fn test_delegate_compressed() {
    // Setup
    let (mut rpc, payer) = setup_program_test_env().await;

    let validator_keypair = Keypair::from_bytes(&TEST_AUTHORITY).unwrap();

    let address_tree_info = rpc.get_address_tree_v1();
    let address_tree_pubkey = address_tree_info.tree;

    let pda = Pubkey::find_program_address(DELEGATED_PDA_SEED, &DELEGATED_PDA_OWNER_ID).0;

    // Create counter
    let (address, _) = derive_address(
        DELEGATED_PDA_SEED,
        &address_tree_pubkey,
        &DELEGATED_PDA_OWNER_ID,
    );
    let merkle_tree_pubkey = rpc.get_random_state_tree_info().unwrap().tree;

    let system_account_meta_config = SystemAccountMetaConfig::new(DELEGATED_PDA_OWNER_ID);
    let mut accounts = PackedAccounts::default();
    accounts.add_pre_accounts_signer(payer.pubkey());
    accounts.add_system_accounts(system_account_meta_config);

    let rpc_result = rpc
        .get_validity_proof(
            vec![],
            vec![AddressWithTree {
                address,
                tree: address_tree_pubkey,
            }],
            None,
        )
        .await
        .unwrap()
        .value;

    let output_merkle_tree_index = accounts.insert_or_get(merkle_tree_pubkey);
    let packed_address_tree_info = rpc_result.pack_tree_infos(&mut accounts).address_trees[0];
    let (accounts, _, _) = accounts.to_account_metas();

    let instruction_data = DelegateCompressedArgs {
        commit_frequency_ms: 0,
        seeds: vec![],
        validator: Some(validator_keypair.pubkey()),
        proof: rpc_result.proof,
        address_tree_info: packed_address_tree_info,
        output_state_tree_index: output_merkle_tree_index,
        account_meta: CompressedAccountMeta::default(),
        account_data: b"some data".to_vec(),
    };
    let inputs = instruction_data.try_to_vec().unwrap();

    eprintln!("payer: {:?}", payer.pubkey());
    eprintln!("pda: {:?}", pda);
    eprintln!("owner: {:?}", DELEGATED_PDA_OWNER_ID);
    eprintln!("merkle_tree_pubkey: {:?}", merkle_tree_pubkey);
    eprintln!(
        "system_account_meta_config: {:?}",
        system_account_meta_config.self_program
    );
    eprintln!("validator_keypair: {:?}", validator_keypair.pubkey());
    eprintln!(
        "accounts: {}",
        accounts
            .iter()
            .map(|a| format!("{:?}", a))
            .collect::<Vec<String>>()
            .join("\n")
    );
    let instruction =
        delegate_compressed_from_wrapper_program(payer.pubkey(), pda, accounts, inputs);

    rpc.create_and_send_transaction(&[instruction], &payer.pubkey(), &[&payer])
        .await
        .unwrap();

    let result = rpc.get_compressed_account(address, None).await.unwrap();
    println!("{:?}", result);
    assert!(result.value.owner.eq(&DELEGATED_PDA_OWNER_ID));

    let compressed_account =
        DelegatedCompressedAccount::try_from_slice(&result.value.data.unwrap().data).unwrap();
    let expected_compressed_account = DelegatedCompressedAccount {
        account_data: b"some data".to_vec(),
        seeds: vec![],
        authority: validator_keypair.pubkey(),
        owner: DELEGATED_PDA_OWNER_ID,
        delegation_slot: 0,
        commit_frequency_ms: 0,
    };
    assert!(compressed_account.eq(&expected_compressed_account));
}

async fn setup_program_test_env() -> (LightProgramTest, Keypair) {
    let cargo_target_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let path = std::path::Path::new(&cargo_target_dir)
        .join("target")
        .join("deploy");
    std::env::set_var("SBF_OUT_DIR", path);
    let config = ProgramTestConfig::new(
        true,
        Some(vec![
            (
                "../../tests/integration/target/deploy/test_delegation",
                DELEGATED_PDA_OWNER_ID,
            ),
            ("dlp", dlp::ID),
        ]),
    );
    let rpc = LightProgramTest::new(config).await.unwrap();
    let payer = rpc.get_payer().insecure_clone();

    (rpc, payer)
}

/// Builds a delegate instruction for the test program
fn delegate_compressed_from_wrapper_program(
    payer: Pubkey,
    delegated_account: Pubkey,
    accounts: Vec<AccountMeta>,
    data: Vec<u8>,
) -> Instruction {
    Instruction {
        program_id: DELEGATED_PDA_OWNER_ID,
        accounts: [
            &[
                AccountMeta::new(payer, true),
                AccountMeta::new_readonly(delegated_account, false),
                AccountMeta::new_readonly(DELEGATED_PDA_OWNER_ID, false),
                AccountMeta::new_readonly(dlp::id(), false),
            ],
            accounts.as_slice(),
        ]
        .concat(),
        data: [
            EXTERNAL_DELEGATE_COMPRESSED_INSTRUCTION_DISCRIMINATOR.to_vec(),
            data,
        ]
        .concat(),
    }
}
