use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    system_program,
};

use crate::{
    args::{
        DelegateArgs, DelegateWithActionsArgs, Instructions,
        PostDelegationActions,
    },
    compact::{self},
    discriminator::DlpDiscriminator,
    pda::{
        delegate_buffer_pda_from_delegated_account_and_owner_program,
        delegation_metadata_pda_from_delegated_account,
        delegation_record_pda_from_delegated_account,
    },
};

/// Builds a delegate instruction that stores an actions payload.
/// See [crate::processor::process_delegate_with_actions] for docs.
pub fn delegate_with_actions(
    payer: Pubkey,
    delegated_account: Pubkey,
    owner: Option<Pubkey>,
    delegate: DelegateArgs,
    actions: Vec<Instruction>,
    private: bool,
) -> Instruction {
    let actions =
        compact_post_delegation_actions(actions, private, delegate.validator);

    Instruction {
        program_id: crate::id(),

        accounts: {
            let owner = owner.unwrap_or(system_program::id());
            let delegate_buffer_pda =
                delegate_buffer_pda_from_delegated_account_and_owner_program(
                    &delegated_account,
                    &owner,
                );
            let delegation_record_pda =
                delegation_record_pda_from_delegated_account(
                    &delegated_account,
                );
            let delegation_metadata_pda =
                delegation_metadata_pda_from_delegated_account(
                    &delegated_account,
                );

            [
                vec![
                    AccountMeta::new(payer, true),
                    AccountMeta::new(delegated_account, true),
                    AccountMeta::new_readonly(owner, false),
                    AccountMeta::new(delegate_buffer_pda, false),
                    AccountMeta::new(delegation_record_pda, false),
                    AccountMeta::new(delegation_metadata_pda, false),
                    AccountMeta::new_readonly(system_program::id(), false),
                ],
                actions
                    .pubkeys
                    .iter()
                    .take(actions.signer_count as usize)
                    .map(|signer| AccountMeta::new_readonly(*signer, true))
                    .collect(),
            ]
            .concat()
        },

        data: {
            let args = DelegateWithActionsArgs { delegate, actions };
            let mut data = DlpDiscriminator::DelegateWithActions.to_vec();
            data.extend_from_slice(&bincode::serialize(&args).unwrap());
            data
        },
    }
}

fn compact_post_delegation_actions(
    instructions: Vec<Instruction>,
    private: bool,
    validator: Option<Pubkey>,
) -> PostDelegationActions {
    let mut pubkeys: Vec<(Pubkey, usize, bool)> = Vec::new(); // Vec of (pubkey, index, signer)

    // return index to pubkeys
    let mut index_of = |key: Pubkey, signer: bool| -> u8 {
        if let Some(index) =
            pubkeys.iter().position(|(existing, _, _)| *existing == key)
        {
            pubkeys[index].2 |= signer;
            return index as u8;
        }
        assert!(
            pubkeys.len() < compact::MAX_PUBKEYS as usize,
            "delegate_with_actions supports at most {} unique pubkeys",
            compact::MAX_PUBKEYS
        );
        pubkeys.push((key, pubkeys.len(), signer));
        pubkeys.len() as u8 - 1
    };

    let compact_instructions = instructions
        .into_iter()
        .map(|ix| compact::Instruction::from_instruction(ix, &mut index_of))
        .collect();

    let (pubkeys, compact_instructions, signer_count) =
        reorder_signers_first(pubkeys, compact_instructions);

    let compact_payload = if private {
        let serialized = bincode::serialize(&compact_instructions)
            .expect("compact instruction serialization should not fail");
        let validator = validator
            .expect("delegate.validator is required when private is true");

        #[cfg(feature = "sdk")]
        {
            Instructions::Encrypted {
                instructions: crate::encryption::encrypt_ed25519_recipient(
                    &serialized,
                    &validator.to_bytes(),
                )
                .expect("validator ed25519 pubkey must convert to x25519"),
            }
        }

        #[cfg(not(feature = "sdk"))]
        {
            let _ = (serialized, validator);
            panic!("private delegate_with_actions requires sdk feature");
        }
    } else {
        Instructions::ClearText {
            instructions: compact_instructions.clone(),
        }
    };

    PostDelegationActions {
        signer_count,
        pubkeys,
        instructions: compact_payload,
    }
}

fn reorder_signers_first(
    mut pubkeys: Vec<(Pubkey, usize, bool)>,
    mut instructions: Vec<compact::Instruction>,
) -> (Vec<Pubkey>, Vec<compact::Instruction>, u8) {
    if pubkeys.is_empty() {
        return (Vec::new(), instructions, 0);
    }

    let signer_count = partition(&mut pubkeys, |(_, _, signer)| *signer);

    let new_index = |old_index: u8| -> u8 {
        pubkeys
            .iter()
            .position(|(_, index, _)| *index == old_index as usize)
            .unwrap() as u8
    };

    for ix in instructions.iter_mut() {
        ix.program_id = new_index(ix.program_id);
        for meta in ix.accounts.iter_mut() {
            meta.set_index(new_index(meta.index()));
        }
    }

    let pubkeys = pubkeys.into_iter().map(|(key, _, _)| key).collect();
    (pubkeys, instructions, signer_count as u8)
}

///
/// It's a C++ equivalent of std::partition()
/// ref: https://en.cppreference.com/w/cpp/algorithm/partition.html
///
/// Returns the size of first group (good elements) which can also be used as the
/// index of the first element in the second group.
///
fn partition<T, F>(v: &mut [T], mut pred: F) -> usize
where
    F: FnMut(&T) -> bool,
{
    let mut good = 0; // number of good elements

    for i in 0..v.len() {
        if pred(&v[i]) {
            v.swap(good, i);
            good += 1;
        }
    }

    good
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reorder_signers_first_remaps_and_prefixes_signers() {
        let a = Pubkey::new_from_array([1; 32]); // 0: signer
        let b = Pubkey::new_from_array([2; 32]); // 1: non-signer
        let c = Pubkey::new_from_array([3; 32]); // 2: signer
        let d = Pubkey::new_from_array([4; 32]); // 3: non-signer
        let e = Pubkey::new_from_array([5; 32]); // 4: signer

        // (pubkey, old_index, is_signer)
        let pubkeys = vec![
            (a, 0, true),
            (b, 1, false),
            (c, 2, true),
            (d, 3, false),
            (e, 4, true),
        ];
        let instructions = vec![compact::Instruction {
            program_id: 3, // old index of d
            accounts: vec![
                compact::AccountMeta::new_readonly(0, true), // a
                compact::AccountMeta::new(2, true),          // c
                compact::AccountMeta::new_readonly(1, false), // b
                compact::AccountMeta::new_readonly(4, true), // e
                compact::AccountMeta::new(3, false),         // d
            ],
            data: vec![9],
        }];

        let (reordered_pubkeys, ixs, signer_count) =
            reorder_signers_first(pubkeys, instructions);

        // reordered: a, c, e, d, b
        //            0, 1, 2, 3, 4

        assert_eq!(signer_count, 3);
        assert_eq!(reordered_pubkeys[0], a); // signer
        assert_eq!(reordered_pubkeys[1], c); // signer
        assert_eq!(reordered_pubkeys[2], e); // signer
        assert_eq!(reordered_pubkeys[3], d); // non-signer
        assert_eq!(reordered_pubkeys[4], b); // non-signer

        // old->new mapping: a(0)->0, b(1)->4, c(2)->1, d(3)->3, e(4)->2
        //
        assert_eq!(ixs[0].program_id, 3); // d
        assert_eq!(ixs[0].accounts[0].index(), 0); // a
        assert_eq!(ixs[0].accounts[1].index(), 1); // c
        assert_eq!(ixs[0].accounts[2].index(), 4); // b
        assert_eq!(ixs[0].accounts[3].index(), 2); // e
        assert_eq!(ixs[0].accounts[4].index(), 3); // d
    }

    #[test]
    fn test_compact_post_delegation_actions() {
        let a = Pubkey::new_from_array([1; 32]); // 0: signer
        let b = Pubkey::new_from_array([2; 32]); // 1: non-signer
        let c = Pubkey::new_from_array([3; 32]); // 2: signer
        let d = Pubkey::new_from_array([4; 32]); // 3: non-signer
        let e = Pubkey::new_from_array([5; 32]); // 4: signer

        let instructions = vec![Instruction {
            program_id: d,
            accounts: vec![
                AccountMeta::new_readonly(a, true),  // a
                AccountMeta::new(c, true),           // c
                AccountMeta::new_readonly(b, false), // b
                AccountMeta::new_readonly(e, true),  // e
                AccountMeta::new(d, false),          // d
            ],
            data: vec![9],
        }];

        let actions =
            compact_post_delegation_actions(instructions, false, None);

        // reordered: a, c, e, d, b
        //            0, 1, 2, 3, 4

        assert_eq!(actions.signer_count, 3);
        assert_eq!(actions.pubkeys[0], a); // signer
        assert_eq!(actions.pubkeys[1], c); // signer
        assert_eq!(actions.pubkeys[2], e); // signer
        assert_eq!(actions.pubkeys[3], b); // non-signer
        assert_eq!(actions.pubkeys[4], d); // non-signer

        // old->new mapping: a(0)->0, b(1)->4, c(2)->1, d(3)->3, e(4)->2
        let Instructions::ClearText { instructions: ixs } =
            actions.instructions
        else {
            panic!();
        };

        assert_eq!(ixs[0].program_id, 4); // d
        assert_eq!(ixs[0].accounts[0].index(), 0); // a
        assert_eq!(ixs[0].accounts[1].index(), 1); // c
        assert_eq!(ixs[0].accounts[2].index(), 3); // b
        assert_eq!(ixs[0].accounts[3].index(), 2); // e
        assert_eq!(ixs[0].accounts[4].index(), 4); // d
    }
}
