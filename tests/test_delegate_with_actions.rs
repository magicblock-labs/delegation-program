use dlp::{
    args::{DelegateArgs, DelegateWithActionsArgs},
    compact,
};
use dlp_api::{
    Decrypt,
    instruction_builder::{
    delegate_with_actions, Encryptable, EncryptableFrom,
    PostDelegationInstruction,
    },
};
use solana_program::{instruction::AccountMeta, pubkey::Pubkey};
use solana_sdk::signer::Signer;

#[test]
fn test_compact_account_meta_bit_packing() {
    let packed = compact::AccountMeta::new_readonly(42, true);
    assert_eq!(packed.key(), 42);
    assert!(packed.is_signer());
    assert!(!packed.is_writable());

    let packed = compact::AccountMeta::new(63, false);
    assert_eq!(packed.key(), 63);
    assert!(!packed.is_signer());
    assert!(packed.is_writable());

    assert!(compact::AccountMeta::try_new(64, true, true).is_none());
}

#[test]
fn test_delegate_with_actions_bincode_roundtrip_compact_payload() {
    let payer = Pubkey::new_unique();
    let signer = Pubkey::new_unique();

    let instructions = vec![
        PostDelegationInstruction {
            program_id: Pubkey::new_unique().cleartext(),
            accounts: vec![
                AccountMeta::new_readonly(payer, true).cleartext(),
                AccountMeta::new(Pubkey::new_unique(), false).cleartext(),
            ],
            data: vec![1, 2, 3].encrypted_from(3),
        },
        PostDelegationInstruction {
            program_id: Pubkey::new_unique().cleartext(),
            accounts: vec![AccountMeta::new_readonly(signer, true).cleartext()],
            data: vec![9, 9].encrypted_from(2),
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
    );

    let args: DelegateWithActionsArgs =
        bincode::deserialize(&ix.data[8..]).unwrap();
    assert_eq!(args.delegate.commit_frequency_ms, 500);
    assert_eq!(args.actions.signers.len(), 2);
    assert_eq!(args.actions.instructions.len(), 2);
    assert!(
        args.actions.signers.len() + args.actions.non_signers.len()
            <= compact::MAX_PUBKEYS as usize
    );
}

#[test]
fn test_delegate_with_actions_builder_adds_compact_signers_to_remaining_accounts(
) {
    use solana_sdk::signature::Keypair;

    let validator = Keypair::new();
    let payer = Pubkey::new_unique();
    let delegated_account = Pubkey::new_unique();
    let owner = Pubkey::new_unique();
    let signer_a = Pubkey::new_unique();
    let signer_b = Pubkey::new_unique();

    let instructions = vec![
        PostDelegationInstruction {
            program_id: Pubkey::new_unique().cleartext(),
            accounts: vec![
                AccountMeta::new_readonly(signer_a, true).cleartext(),
                AccountMeta::new_readonly(signer_b, true).cleartext(),
            ],
            data: vec![7, 7].encrypted_from(2),
        },
        PostDelegationInstruction {
            program_id: Pubkey::new_unique().cleartext(),
            accounts: vec![
                AccountMeta::new_readonly(signer_a, true).cleartext(),
                AccountMeta::new(Pubkey::new_unique(), false).cleartext(),
            ],
            data: vec![8, 8].encrypted_from(2),
        },
    ];

    let ix = delegate_with_actions(
        payer,
        delegated_account,
        Some(owner),
        DelegateArgs {
            validator: Some(validator.pubkey()),
            ..Default::default()
        },
        instructions,
    );

    // first 7 are the required delegate_with_actions accounts
    let remaining = &ix.accounts[7..];
    assert_eq!(remaining.len(), 2);
    assert!(remaining.iter().all(|a| a.is_signer && !a.is_writable));
    assert!(remaining.iter().any(|a| a.pubkey == signer_a));
    assert!(remaining.iter().any(|a| a.pubkey == signer_b));
}

#[test]
fn test_delegate_with_actions_builder_private_sets_encrypted_payload() {
    use dlp_api::encryption;
    use solana_sdk::signature::Keypair;

    let validator = Keypair::new();
    let validator_x25519_secret =
        encryption::keypair_to_x25519_secret(&validator).unwrap();
    let validator_x25519_pubkey =
        encryption::ed25519_pubkey_to_x25519(validator.pubkey().as_array())
            .unwrap();

    let payer = Pubkey::new_unique();
    let signer = Pubkey::new_unique();
    let instructions = vec![PostDelegationInstruction {
        program_id: Pubkey::new_unique().cleartext(),
        accounts: vec![AccountMeta::new_readonly(signer, true).cleartext()],
        data: vec![4, 2].encrypted_from(1),
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
    );

    let args: DelegateWithActionsArgs =
        bincode::deserialize(&ix.data[8..]).unwrap();
    assert_eq!(args.actions.signers.len(), 1);
    let ix = &args.actions.instructions[0];
    assert_eq!(ix.data.prefix, vec![4]);
    let decrypted = encryption::decrypt(
        ix.data.suffix.as_bytes(),
        &validator_x25519_pubkey,
        &validator_x25519_secret,
    )
    .unwrap();
    assert_eq!(decrypted, vec![2]);
}

#[test]
fn test_delegate_with_actions_builder_encrypts_and_decrypts_accounts_and_data() {
    use solana_sdk::signature::Keypair;

    let validator = Keypair::new();
    let payer = Pubkey::new_unique();
    let signer = Pubkey::new_unique();
    let nonsigner = Pubkey::new_unique();
    let program_id = Pubkey::new_unique();

    let instructions = vec![PostDelegationInstruction {
        program_id: program_id.cleartext(),
        accounts: vec![
            AccountMeta::new_readonly(signer, true).cleartext(),
            AccountMeta::new_readonly(nonsigner, false).encrypted(),
        ],
        data: vec![7, 8, 9].encrypted_from(1),
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
    );

    let args: DelegateWithActionsArgs =
        bincode::deserialize(&ix.data[8..]).unwrap();
    let ix = &args.actions.instructions[0];

    let clear_meta = match ix.accounts[0] {
        dlp::args::MaybeEncryptedAccountMeta::ClearText(meta) => meta,
        _ => panic!("expected cleartext account meta for signer"),
    };
    assert_eq!(clear_meta.key(), 0);
    assert!(clear_meta.is_signer());
    assert!(!clear_meta.is_writable());

    let decrypted_meta = ix.accounts[1]
        .clone()
        .decrypt_with_keypair(&validator)
        .unwrap();
    assert_eq!(decrypted_meta.key(), 2);
    assert!(!decrypted_meta.is_signer());
    assert!(!decrypted_meta.is_writable());

    let decrypted_data = ix
        .data
        .clone()
        .decrypt_with_keypair(&validator)
        .unwrap();
    assert_eq!(decrypted_data, vec![7, 8, 9]);
}
