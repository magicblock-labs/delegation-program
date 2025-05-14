use solana_program::pubkey::Pubkey;

/// The delegation session fees (extracted in percentage from the delegation PDAs rent on closure).
pub const RENT_FEES_PERCENTAGE: u8 = 10;

/// The fees extracted from the validator earnings (extracted in percentage from the validator fees claims).
pub const PROTOCOL_FEES_PERCENTAGE: u8 = 10;

/// The discriminator for the external undelegate instruction.
pub const EXTERNAL_UNDELEGATE_DISCRIMINATOR: [u8; 8] = [196, 28, 41, 206, 48, 37, 51, 167];

/// The program ID of the delegation program.
pub const DELEGATION_PROGRAM_ID: Pubkey = crate::id();

/// The seed of the authority account PDA.
pub const DELEGATION_RECORD: &[u8] = b"delegation";

/// The account to store the delegated account seeds.
pub const DELEGATION_METADATA: &[u8] = b"delegation-metadata";

/// The seed of the committed state PDA.
pub const COMMIT_STATE: &[u8] = b"state-diff";

/// The seed of a commit state record PDA.
pub const COMMIT_STATE_RECORD: &[u8] = b"commit-state-record";

/// The seed of the buffer account PDA.
pub const BUFFER: &[u8] = b"buffer";

/// The seed of undelegate buffer account PDA
pub const UNDELEGATE_BUFFER: &[u8] = b"undelegate-buffer";

/// The seed of fees vault PDA
pub const FEES_VAULT: &[u8] = b"fees-vault";

/// The seed of validator fees vault PDA
pub const VALIDATOR_FEES_VAULT: &[u8] = b"v-fees-vault";

/// The seed of program config PDA
pub const PROGRAM_CONFIG: &[u8] = b"p-conf";

/// The seed of ephemeral balance PDA
pub const EPHEMERAL_BALANCE: &[u8] = b"balance";
