use solana_program::pubkey;
use solana_program::pubkey::Pubkey;

/// The delegation session fees (extracted in percentage from the delegation PDAs rent on closure).
pub const FEES_SESSION: u8 = 30;

/// The fees extracted from the validator earnings (extracted in percentage from the validator fees claims).
pub const FEES_VOLUME: u8 = 10;

/// The seed of the authority account PDA.
pub const DELEGATION_RECORD: &[u8] = b"delegation";
pub const DELEGATION_RECORD_DISCRIMINANT: &[u8; 8] = &[100, 0, 0, 0, 0, 0, 0, 0];

/// The account to store the delegated account seeds.
pub const DELEGATION_METADATA: &[u8] = b"delegation-metadata";
pub const DELEGATION_METADATA_DISCRIMINANT: &[u8; 8] = &[102, 0, 0, 0, 0, 0, 0, 0];

/// The seed of the buffer account PDA.
pub const BUFFER: &[u8] = b"buffer";

/// The seed of the committed state PDA.
pub const COMMIT_STATE: &[u8] = b"state-diff";

/// The seed of a commit state record PDA.
pub const COMMIT_RECORD: &[u8] = b"commit-state-record";
pub const COMMIT_RECORD_DISCRIMINANT: &[u8; 8] = &[101, 0, 0, 0, 0, 0, 0, 0];

/// The account to store lamports deposited for paying fees.
pub const FEES_VAULT: &[u8] = b"fees-vault";

/// The account to store the validator fees vault PDA.
pub const VALIDATOR_FEES_VAULT: &[u8] = b"v-fees-vault";

/// The account to store the program config (e.g. whitelisting of validators) PDA.
pub const PROGRAM_CONFIG: &[u8] = b"p-conf";
pub const PROGRAM_CONFIG_DISCRIMINANT: &[u8; 8] = &[103, 0, 0, 0, 0, 0, 0, 0];

/// A Pda used to escrow lamports in the ephemeral validator.
pub const EPHEMERAL_BALANCE: &[u8] = b"balance";

/// The discriminator for the external undelegate instruction.
pub const EXTERNAL_UNDELEGATE_DISCRIMINATOR: [u8; 8] = [196, 28, 41, 206, 48, 37, 51, 167];

/// The program ID of the delegation program.
pub const DELEGATION_PROGRAM_ID: Pubkey = crate::id();

/// The admin pubkey of the authority allowed to whitelist validators.
#[cfg(feature = "unit_test_config")]
pub const ADMIN_PUBKEY: Pubkey = pubkey!("tEsT3eV6RFCWs1BZ7AXTzasHqTtMnMLCB2tjQ42TDXD");
#[cfg(not(feature = "unit_test_config"))]
pub const ADMIN_PUBKEY: Pubkey = pubkey!("3FwNxjbCqdD7G6MkrAdwTd5Zf6R3tHoapam4Pv1X2KBB");
