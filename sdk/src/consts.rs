// NOTE: this should go into a core package that both the sdk + the program can depend on
use solana_program::pubkey;
use solana_program::pubkey::Pubkey;

/// The delegation program ID.
pub const DELEGATION_PROGRAM_ID: Pubkey = pubkey!("DELeGGvXpWV2fqJUhqcF5ZSYMS4JTLjteaAMARRSaeSh");

/// The magic program ID.
pub const MAGIC_PROGRAM_ID: Pubkey = pubkey!("Magic11111111111111111111111111111111111111");
///
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

/// The discriminator for the external undelegate instruction.
pub const EXTERNAL_UNDELEGATE_DISCRIMINATOR: [u8; 8] = [196, 28, 41, 206, 48, 37, 51, 167];
