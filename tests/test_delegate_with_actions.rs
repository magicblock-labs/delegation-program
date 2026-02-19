use dlp::{
    args::{DelegateArgs, DelegateWithActionsArgs, Instructions},
    compact,
    instruction_builder::delegate_with_actions,
};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

#[test]
fn test_compact_account_meta_bit_packing() {
    let packed = compact::AccountMeta::new_readonly(42, true);
    assert_eq!(packed.index(), 42);
    assert!(packed.is_signer());
    assert!(!packed.is_writable());

    let packed = compact::AccountMeta::new(63, false);
    assert_eq!(packed.index(), 63);
    assert!(!packed.is_signer());
    assert!(packed.is_writable());

    assert!(compact::AccountMeta::try_new(64, true, true).is_none());
}

#[test]
fn test_delegate_with_actions_bincode_roundtrip_compact_payload() {
    let payer = Pubkey::new_unique();
    let signer = Pubkey::new_unique();

    let instructions = vec![
        Instruction {
            program_id: Pubkey::new_unique(),
            accounts: vec![
                AccountMeta::new_readonly(payer, true),
                AccountMeta::new(Pubkey::new_unique(), false),
            ],
            data: vec![1, 2, 3],
        },
        Instruction {
            program_id: Pubkey::new_unique(),
            accounts: vec![AccountMeta::new_readonly(signer, true)],
            data: vec![9, 9],
        },
    ];

    let ix = delegate_with_actions(
        payer,
        Pubkey::new_unique(),
        Some(Pubkey::new_unique()),
        DelegateArgs {
            commit_frequency_ms: 500,
            seeds: vec![b"seed-a".to_vec()],
            validator: Some(Pubkey::new_unique()),
        },
        instructions,
        false,
    );

    let args: DelegateWithActionsArgs =
        bincode::deserialize(&ix.data[8..]).unwrap();
    assert_eq!(args.delegate.commit_frequency_ms, 500);
    assert_eq!(args.actions.signer_count, 2);
    match args.actions.instructions {
        Instructions::ClearText { instructions } => {
            assert_eq!(instructions.len(), 2);
        }
        Instructions::Encrypted { .. } => {
            panic!("expected cleartext compact instructions");
        }
    }
    assert!(args.actions.pubkeys.len() <= compact::MAX_PUBKEYS as usize);
}

#[test]
fn test_delegate_with_actions_builder_adds_compact_signers_to_remaining_accounts(
) {
    let payer = Pubkey::new_unique();
    let delegated_account = Pubkey::new_unique();
    let owner = Pubkey::new_unique();
    let signer_a = Pubkey::new_unique();
    let signer_b = Pubkey::new_unique();

    let instructions = vec![
        Instruction {
            program_id: Pubkey::new_unique(),
            accounts: vec![
                AccountMeta::new_readonly(signer_a, true),
                AccountMeta::new_readonly(signer_b, true),
            ],
            data: vec![7, 7],
        },
        Instruction {
            program_id: Pubkey::new_unique(),
            accounts: vec![
                AccountMeta::new_readonly(signer_a, true),
                AccountMeta::new(Pubkey::new_unique(), false),
            ],
            data: vec![8, 8],
        },
    ];

    let ix = delegate_with_actions(
        payer,
        delegated_account,
        Some(owner),
        DelegateArgs::default(),
        instructions,
        false,
    );

    // first 7 are the required delegate_with_actions accounts
    let remaining = &ix.accounts[7..];
    assert_eq!(remaining.len(), 2);
    assert!(remaining.iter().all(|a| a.is_signer && !a.is_writable));
    assert!(remaining.iter().any(|a| a.pubkey == signer_a));
    assert!(remaining.iter().any(|a| a.pubkey == signer_b));
}

#[test]
#[cfg(feature = "sdk")]
fn test_delegate_with_actions_builder_private_sets_encrypted_payload() {
    use dlp::encryption;
    use solana_sdk::signature::Keypair;

    let validator = Keypair::new();
    let validator_secret = encryption::keypair_to_x25519_secret(&validator);

    let payer = Pubkey::new_unique();
    let signer = Pubkey::new_unique();
    let instructions = vec![Instruction {
        program_id: Pubkey::new_unique(),
        accounts: vec![AccountMeta::new_readonly(signer, true)],
        data: vec![4, 2],
    }];

    let ix = delegate_with_actions(
        payer,
        Pubkey::new_unique(),
        Some(Pubkey::new_unique()),
        DelegateArgs {
            validator: Some(validator.pubkey()),
            ..Default::default()
        },
        instructions,
        true,
    );

    let args: DelegateWithActionsArgs =
        bincode::deserialize(&ix.data[8..]).unwrap();
    assert_eq!(args.actions.signer_count, 1);
    match args.actions.instructions {
        Instructions::Encrypted { instructions } => {
            let decrypted =
                encryption::decrypt(&instructions, &validator_secret).unwrap();
            let decoded: Vec<dlp::compact::Instruction> =
                bincode::deserialize(&decrypted).unwrap();
            assert_eq!(decoded.len(), 1);
            assert_eq!(decoded[0].data, vec![4, 2]);
        }
        Instructions::ClearText { .. } => {
            panic!("expected encrypted compact instructions");
        }
    }
}
