use solana_program::pubkey::Pubkey;
use solana_sdk::pubkey;

#[allow(dead_code)]
pub const DELEGATION_RECORD_ACCOUNT_DATA: [u8; 80] = [
    100, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 43, 85, 175, 207, 195, 148, 154, 129, 218, 62, 110, 177, 81, 112,
    72, 172, 141, 157, 3, 211, 24, 26, 191, 79, 101, 191, 48, 19, 105, 181, 70, 132, 48, 117, 0, 0,
    0, 0, 0, 0,
];

#[allow(dead_code)]
pub const COMMIT_STATE_RECORD_ACCOUNT_DATA: [u8; 80] = [
    101, 0, 0, 0, 0, 0, 0, 0, 202, 37, 188, 175, 199, 216, 218, 84, 43, 75, 255, 157, 215, 202,
    195, 114, 139, 194, 225, 131, 177, 111, 103, 238, 162, 225, 196, 178, 29, 219, 96, 127, 115, 7,
    118, 65, 61, 170, 109, 216, 57, 214, 57, 150, 28, 32, 145, 234, 70, 215, 243, 242, 145, 103,
    150, 11, 142, 149, 177, 109, 222, 157, 148, 7, 26, 0, 0, 0, 0, 0, 0, 0,
];

#[allow(dead_code)]
pub const DELEGATION_METADATA_PDA: [u8; 41] = [
    0, 0, 0, 0, 0, 0, 0, 0, 28, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 8, 0,
    0, 0, 116, 101, 115, 116, 45, 112, 100, 97,
];

#[allow(dead_code)]
pub const DELEGATION_METADATA_UNDELEGATABLE_PDA: [u8; 41] = [
    0, 0, 0, 0, 0, 0, 0, 0, 98, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 8, 0,
    0, 0, 116, 101, 115, 116, 45, 112, 100, 97,
];

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
