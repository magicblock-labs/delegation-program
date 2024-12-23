use solana_program::pubkey;
use solana_program::pubkey::Pubkey;

/// The delegation session fees (extracted in percentage from the delegation PDAs rent on closure).
pub const FEES_SESSION: u8 = 30;

/// The fees extracted from the validator earnings (extracted in percentage from the validator fees claims).
pub const FEES_VOLUME: u8 = 10;

/// The discriminator for the external undelegate instruction.
pub const EXTERNAL_UNDELEGATE_DISCRIMINATOR: [u8; 8] = [196, 28, 41, 206, 48, 37, 51, 167];

/// The program ID of the delegation program.
pub const DELEGATION_PROGRAM_ID: Pubkey = crate::id();

/// The admin pubkey of the authority allowed to whitelist validators.
#[cfg(feature = "unit_test_config")]
pub const ADMIN_PUBKEY: Pubkey = pubkey!("tEsT3eV6RFCWs1BZ7AXTzasHqTtMnMLCB2tjQ42TDXD");
#[cfg(not(feature = "unit_test_config"))]
pub const ADMIN_PUBKEY: Pubkey = pubkey!("3FwNxjbCqdD7G6MkrAdwTd5Zf6R3tHoapam4Pv1X2KBB");
