pub mod accounts;

use std::collections::hash_map::Entry;
use std::collections::HashMap;

#[allow(unused_imports)]
pub(crate) use accounts::*;

use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey};
use solana_sdk::entrypoint::{deserialize, NON_DUP_MARKER};

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    // 1️⃣ Build the serialized buffer that entrypoint() expects
    let mut serialized_input = serialize(program_id, accounts, instruction_data);

    test_serialize(&serialized_input, (program_id, accounts, instruction_data));

    // 2️⃣ Call the real entrypoint just like the runtime would
    let rc = unsafe { dlp::entrypoint(serialized_input.as_mut_ptr()) };

    // 3️⃣ Convert return code to Result
    if rc == pinocchio::SUCCESS {
        Ok(())
    } else {
        Err(solana_program::program_error::ProgramError::Custom(
            rc as u32,
        ))
    }
}

fn test_serialize(input: &[u8], expected: (&Pubkey, &[AccountInfo], &[u8])) {
    let mut used_input = input.to_vec();
    let actual = unsafe { deserialize(used_input.as_mut_ptr()) };

    assert_eq!(
        actual.0, expected.0,
        "actual: {}, expected: {}",
        actual.0, expected.0
    );

    assert_eq!(
        actual.1.len(),
        expected.1.len(),
        "actual (len): {}, expected (len): {}",
        actual.1.len(),
        expected.1.len(),
    );

    for (a, b) in actual.1.iter().zip(expected.1) {
        test_account_eq(a, b);
    }

    assert_eq!(
        actual.2,
        expected.2,
        "actual (len): {}, expected (len): {}",
        actual.2.len(),
        expected.2.len()
    );
}

fn test_account_eq(a: &AccountInfo, b: &AccountInfo) {
    todo!();
}

fn serialize(program_id: &Pubkey, accounts: &[AccountInfo], instruction_data: &[u8]) -> Vec<u8> {
    let mut input = vec![];

    // Number of accounts present
    input.extend_from_slice(&(accounts.len() as u64).to_le_bytes());

    // Account Infos
    let mut dups = HashMap::new();
    for (i, account) in accounts.iter().enumerate() {
        match dups.entry(account.key) {
            Entry::Vacant(v) => {
                v.insert(i); // cache the index, to detect duplicate
                input.extend_from_slice(&[NON_DUP_MARKER]);
                serialize_account_info(&mut input, account);
            }
            Entry::Occupied(o) => {
                let found = *o.get();
                input.extend_from_slice(&[found as u8]);
                input.extend_from_slice(&[0u8; 7]);
            }
        }
    }
    // Instruction data

    // Program Id
    input.extend_from_slice(program_id.as_array());

    input
}

fn serialize_account_info<'a>(input: &mut Vec<u8>, account: &AccountInfo<'a>) {
    input.push(if account.is_signer { 1 } else { 0 });
    input.push(if account.is_writable { 1 } else { 0 });
    input.push(if account.executable { 1 } else { 0 });

    // data-len saved here as u32
    input.extend_from_slice(&(account.data_len() as u32).to_le_bytes());

    input.extend_from_slice(account.key.as_array());
    input.extend_from_slice(account.owner.as_array());
    input.extend_from_slice(&account.lamports().to_le_bytes());
    input.extend_from_slice(&(account.data_len() as u64).to_le_bytes());

    input.extend_from_slice(account.data.as_ref().take());

    input.extend_from_slice(&account.rent_epoch.to_le_bytes());
}
