use pinocchio::Address;

use crate::compat::{pubkey, Pubkey};

/// Legacy rent-fee percentage kept for API compatibility.
pub const RENT_FEES_PERCENTAGE: u8 = 10;

/// The fees extracted from the validator earnings (extracted in percentage from the validator fees claims).
pub const PROTOCOL_FEES_PERCENTAGE: u8 = 10;

/// Fixed fee per commit after the first.
pub const COMMIT_FEE_LAMPORTS: u64 = 1_000_000;

/// Fixed fee per delegation session.
pub const SESSION_FEE_LAMPORTS: u64 = 3_000_000;

/// Default and minimum timeout for requested undelegation.
/// Assuming 1 slot is roughly 400ms, then 9000 slots = 60min.
pub const DEFAULT_UNDELEGATION_REQUEST_TIMEOUT_SLOTS: u64 = 9000;

/// The discriminator for the external undelegate instruction.
pub const EXTERNAL_UNDELEGATE_DISCRIMINATOR: [u8; 8] =
    [196, 28, 41, 206, 48, 37, 51, 167];

/// The program ID of the delegation program.
pub const DELEGATION_PROGRAM_ID: Pubkey = crate::id();

/// Default validator identity (used when none is provided during delegation).
#[cfg(not(feature = "unit_test_config"))]
pub const DEFAULT_VALIDATOR_IDENTITY: Pubkey =
    pubkey!("MAS1Dt9qreoRMQ14YQuhg8UTZMMzDdKhmkZMECCzk57");

#[cfg(feature = "unit_test_config")]
pub const DEFAULT_VALIDATOR_IDENTITY: Pubkey =
    pubkey!("tEsT3eV6RFCWs1BZ7AXTzasHqTtMnMLCB2tjQ42TDXD");

/// The broadcast identity marks an account as undelegatable.
/// Validators treat it as always delegatable, which is safe since such accounts
/// cannot be committed or delegated
pub const BROADCAST_IDENTITY: Pubkey =
    pubkey!("Broadcast1111111111111111111111111111111111");

pub const BPF_LOADER_UPGRADEABLE_ID: Address =
    Address::new_from_array(const_crypto::bs58::decode_pubkey(
        "BPFLoaderUpgradeab1e11111111111111111111111",
    ));

pub const DELEGATION_PROGRAM_DATA_ID: Address = Address::new_from_array(
    const_crypto::ed25519::derive_program_address(
        &[crate::fast::ID.as_array()],
        BPF_LOADER_UPGRADEABLE_ID.as_array(),
    )
    .0,
);

pub const RENT_EXCEPTION_ZERO_BYTES_LAMPORTS: u64 = 890880;
