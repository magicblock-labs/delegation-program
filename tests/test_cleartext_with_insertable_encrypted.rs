use dlp_api::{
    compact::ClearTextWithInsertable,
    instruction_builder::{
        Encrypt, Encryptable, EncryptableFrom, PostDelegationInstruction,
    },
    Decrypt,
};
use solana_instruction::{AccountMeta as IxAccountMeta, Instruction};
use solana_program::{
    instruction::{
        AccountMeta as ProgramAccountMeta, Instruction as ProgramInstruction,
    },
    pubkey::Pubkey as ProgramPubkey,
};
use solana_pubkey::Pubkey as IxPubkey;
use solana_sdk::signature::{Keypair, Signer};

fn pk(byte: u8) -> [u8; 32] {
    [byte; 32]
}

#[test]
fn test_cleartext_with_insertable_encrypted_actions() {
    let validator = Keypair::new();
    let validator_pubkey =
        ProgramPubkey::new_from_array(validator.pubkey().to_bytes());

    // Off-chain: user builds a regular instruction and then chooses which parts
    // should be encrypted by converting to PostDelegationInstruction.
    let insert_program = ProgramPubkey::new_from_array(pk(10));
    let s1 = ProgramPubkey::new_from_array(pk(1));
    let s2 = ProgramPubkey::new_from_array(pk(2));
    let n1 = ProgramPubkey::new_from_array(pk(3));
    let n2 = ProgramPubkey::new_from_array(pk(4));
    let n3 = ProgramPubkey::new_from_array(pk(5));

    let insert_ix = PostDelegationInstruction {
        // Encrypt program id to keep cleartext keys total at 4 (2 signers + 2 non-signers).
        program_id: insert_program.encrypted(),
        accounts: vec![
            ProgramAccountMeta::new_readonly(s1, true).cleartext(),
            ProgramAccountMeta::new_readonly(s2, true).cleartext(),
            ProgramAccountMeta::new_readonly(n1, false).cleartext(),
            ProgramAccountMeta::new_readonly(n2, false).cleartext(),
            ProgramAccountMeta::new_readonly(n3, false).encrypted(),
        ],
        data: vec![9, 9, 9].encrypted_from(1),
    };

    let (insertable, _) = vec![insert_ix]
        .encrypt(&validator_pubkey)
        .expect("post-delegation actions encryption failed");

    // On-chain: insert the encrypted actions between two cleartext instructions.
    let actions = vec![
        Instruction {
            program_id: IxPubkey::new_from_array(pk(20)),
            accounts: vec![
                IxAccountMeta::new_readonly(
                    IxPubkey::new_from_array(pk(21)),
                    true,
                ),
                IxAccountMeta::new_readonly(
                    IxPubkey::new_from_array(pk(22)),
                    false,
                ),
            ],
            data: vec![1, 2, 3],
        },
        Instruction {
            program_id: IxPubkey::new_from_array(pk(30)),
            accounts: vec![
                IxAccountMeta::new_readonly(
                    IxPubkey::new_from_array(pk(31)),
                    true,
                ),
                IxAccountMeta::new_readonly(
                    IxPubkey::new_from_array(pk(32)),
                    false,
                ),
            ],
            data: vec![4, 5, 6],
        },
    ]
    .cleartext_with_insertable(insertable, 1);

    assert_eq!(actions.inserted_signers, 2);
    assert_eq!(actions.inserted_non_signers, 4);
    assert_eq!(actions.instructions.len(), 3);

    let is_encrypted = |ix: &dlp_api::args::MaybeEncryptedInstruction| {
        !ix.data.suffix.as_bytes().is_empty()
    };

    assert!(!is_encrypted(&actions.instructions[0]));
    assert!(is_encrypted(&actions.instructions[1]));
    assert!(!is_encrypted(&actions.instructions[2]));

    let decrypted = actions.decrypt_with_keypair(&validator).unwrap();
    let expected = vec![
        ProgramInstruction {
            program_id: ProgramPubkey::new_from_array(pk(20)),
            accounts: vec![
                ProgramAccountMeta::new_readonly(
                    ProgramPubkey::new_from_array(pk(21)),
                    true,
                ),
                ProgramAccountMeta::new_readonly(
                    ProgramPubkey::new_from_array(pk(22)),
                    false,
                ),
            ],
            data: vec![1, 2, 3],
        },
        ProgramInstruction {
            program_id: insert_program,
            accounts: vec![
                ProgramAccountMeta::new_readonly(s1, true),
                ProgramAccountMeta::new_readonly(s2, true),
                ProgramAccountMeta::new_readonly(n1, false),
                ProgramAccountMeta::new_readonly(n2, false),
                ProgramAccountMeta::new_readonly(n3, false),
            ],
            data: vec![9, 9, 9],
        },
        ProgramInstruction {
            program_id: ProgramPubkey::new_from_array(pk(30)),
            accounts: vec![
                ProgramAccountMeta::new_readonly(
                    ProgramPubkey::new_from_array(pk(31)),
                    true,
                ),
                ProgramAccountMeta::new_readonly(
                    ProgramPubkey::new_from_array(pk(32)),
                    false,
                ),
            ],
            data: vec![4, 5, 6],
        },
    ];

    assert_eq!(decrypted, expected);
}
