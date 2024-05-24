use solana_program::pubkey::Pubkey;

/// The seed of the authority account PDA.
pub const DELEGATION_RECORD: &[u8] = b"delegation";

/// The account to store the delegated account seeds.
pub const DELEGATED_ACCOUNT_SEEDS: &[u8] = b"account-seeds";

/// The seed of the buffer account PDA.
pub const BUFFER: &[u8] = b"buffer";

/// The seed of the committed state PDA.
pub const COMMIT_STATE: &[u8] = b"state-diff";

/// The seed of a commit state record PDA.
pub const COMMIT_RECORD: &[u8] = b"commit-state-record";

/// The discriminator for the external undelegate instruction.
pub const EXTERNAL_UNDELEGATE_DISCRIMINATOR: [u8; 8] = [196, 28, 41, 206, 48, 37, 51, 167];

/// The program ID of the delegation program.
pub const DELEGATION_PROGRAM_ID: Pubkey = crate::id();
