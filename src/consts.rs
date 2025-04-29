use solana_program::pubkey::Pubkey;

/// The delegation session fees (extracted in percentage from the delegation PDAs rent on closure).
pub const RENT_FEES_PERCENTAGE: u8 = 10;

/// The fees extracted from the validator earnings (extracted in percentage from the validator fees claims).
pub const PROTOCOL_FEES_PERCENTAGE: u8 = 10;

/// The discriminator for the external undelegate instruction.
pub const EXTERNAL_UNDELEGATE_DISCRIMINATOR: [u8; 8] = [196, 28, 41, 206, 48, 37, 51, 167];

/// The discriminator for the external undelegate instruction.
pub const EXTERNAL_FINALIZE_WITH_HOOK_DISCRIMINATOR: [u8; 8] =
    [74, 203, 100, 144, 173, 103, 210, 31];

/// The program ID of the delegation program.
pub const DELEGATION_PROGRAM_ID: Pubkey = crate::id();
