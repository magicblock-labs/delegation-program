use borsh::BorshSerialize;
use dlp::state::account::AccountWithDiscriminator;
use dlp::state::{CommitRecord, DelegationMetadata, DelegationRecord};
use solana_program::native_token::LAMPORTS_PER_SOL;
use solana_program::pubkey::Pubkey;
use solana_program::rent::Rent;
use solana_program::system_program;
use solana_sdk::pubkey;

// Constants for default values
const DEFAULT_DELEGATION_SLOT: u64 = 0;
const DEFAULT_COMMIT_FREQUENCY_MS: u64 = 0;
const DEFAULT_VALID_UNTIL: i64 = 0;
const DEFAULT_LAST_UPDATE_EXTERNAL_SLOT: u64 = 0;
const DEFAULT_IS_UNDELEGATABLE: bool = false;
const DEFAULT_SEEDS: &[&[u8]] = &[&[116, 101, 115, 116, 45, 112, 100, 97]];

#[allow(dead_code)]
pub const COMMIT_STATE_AUTHORITY: Pubkey = pubkey!("Ec6jL2GVTzjfHz8RFP3mVyki9JRNmMu8E7YdNh45xNdk");

#[allow(dead_code)]
pub const COMMIT_NEW_STATE_ACCOUNT_DATA: [u8; 11] = [10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 11];

#[allow(dead_code)]
pub const DELEGATED_PDA_ID: Pubkey = pubkey!("8k2V7EzQtNg38Gi9HK5ZtQYp1YpGKNGrMcuGa737gZX4");

#[allow(dead_code)]
pub const DELEGATED_PDA: [u8; 19] = [15, 0, 0, 0, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5];

#[allow(dead_code)]
pub const DELEGATED_PDA_OWNER_ID: Pubkey = pubkey!("3vAK9JQiDsKoQNwmcfeEng4Cnv22pYuj1ASfso7U4ukF");

#[allow(dead_code)]
pub const EXTERNAL_DELEGATE_INSTRUCTION_DISCRIMINATOR: [u8; 8] = [90, 147, 75, 178, 85, 88, 4, 137];

#[allow(dead_code)]
pub const EXTERNAL_ALLOW_UNDELEGATION_INSTRUCTION_DISCRIMINATOR: [u8; 8] =
    [255, 66, 82, 208, 247, 5, 210, 126];

#[allow(dead_code)]
pub const ON_CURVE_KEYPAIR: [u8; 64] = [
    74, 198, 48, 104, 119, 57, 255, 80, 67, 181, 191, 189, 85, 21, 235, 45, 185, 175, 48, 143, 13,
    202, 92, 81, 211, 108, 61, 237, 183, 116, 207, 45, 170, 118, 238, 247, 128, 91, 3, 41, 33, 10,
    241, 163, 185, 198, 228, 172, 200, 220, 225, 192, 149, 94, 106, 209, 65, 79, 210, 54, 191, 49,
    115, 159,
];

#[allow(dead_code)]
pub const TEST_AUTHORITY: [u8; 64] = [
    251, 62, 129, 184, 107, 49, 62, 184, 1, 147, 178, 128, 185, 157, 247, 92, 56, 158, 145, 53, 51,
    226, 202, 96, 178, 248, 195, 133, 133, 237, 237, 146, 13, 32, 77, 204, 244, 56, 166, 172, 66,
    113, 150, 218, 112, 42, 110, 181, 98, 158, 222, 194, 130, 93, 175, 100, 190, 106, 9, 69, 156,
    80, 96, 72,
];

#[allow(dead_code)]
pub fn get_delegation_record_data(authority: Pubkey, last_update_lamports: Option<u64>) -> Vec<u8> {
    create_delegation_record_data(authority, DELEGATED_PDA_OWNER_ID, last_update_lamports)
}

#[allow(dead_code)]
pub fn get_delegation_record_on_curve_data(
    authority: Pubkey,
    last_update_lamports: Option<u64>,
) -> Vec<u8> {
    create_delegation_record_data(authority, system_program::id(), last_update_lamports)
}

#[allow(dead_code)]
pub fn create_delegation_record_data(
    authority: Pubkey,
    owner: Pubkey,
    last_update_lamports: Option<u64>,
) -> Vec<u8> {
    let delegation_record = DelegationRecord {
        authority,
        owner,
        delegation_slot: DEFAULT_DELEGATION_SLOT,
        commit_frequency_ms: DEFAULT_COMMIT_FREQUENCY_MS,
        lamports: last_update_lamports.unwrap_or(Rent::default().minimum_balance(500)),
    };
    [
        &DelegationRecord::discriminator().to_bytes(),
        DelegationRecord::to_bytes(&delegation_record),
    ]
    .concat()
}

#[allow(dead_code)]
pub fn get_delegation_metadata_data_on_curve(
    rent_payer: Pubkey,
    is_undelegatable: Option<bool>,
) -> Vec<u8> {
    create_delegation_metadata_data(
        rent_payer,
        &[],
        is_undelegatable.unwrap_or(DEFAULT_IS_UNDELEGATABLE),
    )
}

#[allow(dead_code)]
pub fn get_delegation_metadata_data(rent_payer: Pubkey, is_undelegatable: Option<bool>) -> Vec<u8> {
    create_delegation_metadata_data(
        rent_payer,
        DEFAULT_SEEDS,
        is_undelegatable.unwrap_or(DEFAULT_IS_UNDELEGATABLE),
    )
}

pub fn create_delegation_metadata_data(
    rent_payer: Pubkey,
    seeds: &[&[u8]],
    is_undelegatable: bool,
) -> Vec<u8> {
    let delegation_metadata = DelegationMetadata {
        valid_until: DEFAULT_VALID_UNTIL,
        last_update_external_slot: DEFAULT_LAST_UPDATE_EXTERNAL_SLOT,
        is_undelegatable,
        seeds: seeds.iter().map(|s| s.to_vec()).collect(),
        rent_payer,
    };
    delegation_metadata.try_to_vec().unwrap()
}

#[allow(dead_code)]
pub fn get_commit_record_account_data(authority: Pubkey) -> Vec<u8> {
    let commit_record = CommitRecord {
        slot: 100,
        identity: authority,
        account: DELEGATED_PDA_ID,
        lamports: LAMPORTS_PER_SOL,
    };
    [
        &CommitRecord::discriminator().to_bytes(),
        CommitRecord::to_bytes(&commit_record),
    ]
    .concat()
}
