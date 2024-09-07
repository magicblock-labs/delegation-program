use solana_program::pubkey;
use solana_program::pubkey::Pubkey;

/// The seed of the authority account PDA.
pub const DELEGATION_RECORD: &[u8] = b"delegation";

/// The account to store the delegated account seeds.
pub const DELEGATION_METADATA: &[u8] = b"delegation-metadata";

/// The seed of the buffer account PDA.
pub const BUFFER: &[u8] = b"buffer";

/// The seed of the committed state PDA.
pub const COMMIT_STATE: &[u8] = b"state-diff";

/// The seed of a commit state record PDA.
pub const COMMIT_RECORD: &[u8] = b"commit-state-record";

/// The account to store lamports deposited for paying fees.
pub const FEES_VAULT: &[u8] = b"fees-vault";
pub const WHITELIST: &[u8] = b"whitelist";

/// The account to store ephemeral lamports deposited for a user.
pub const EPHEMERAL_BALANCE: &[u8] = b"ephemeral-balance";

/// The discriminator for the external undelegate instruction.
pub const EXTERNAL_UNDELEGATE_DISCRIMINATOR: [u8; 8] = [196, 28, 41, 206, 48, 37, 51, 167];

/// The program ID of the delegation program.
pub const DELEGATION_PROGRAM_ID: Pubkey = crate::id();

/// The admin pubkey of the authority allowed to whitelist validators.
#[cfg(feature = "unit_test_config")]
pub const ADMIN_PUBKEY: Pubkey = pubkey!("3FwNxjbCqdD7G6MkrAdwTd5Zf6R3tHoapam4Pv1X2KBB");
#[cfg(not(feature = "unit_test_config"))]
pub const ADMIN_PUBKEY: Pubkey = pubkey!("tstp2WEvNF7UATHSPBZCSrNC4cqV2Wr6yhtXveouCWn");
