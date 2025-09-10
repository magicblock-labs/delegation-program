use solana_program::pubkey;
use solana_program::pubkey::Pubkey;

/// The delegation session fees (extracted in percentage from the delegation PDAs rent on closure).
pub const RENT_FEES_PERCENTAGE: u8 = 10;

/// The fees extracted from the validator earnings (extracted in percentage from the validator fees claims).
pub const PROTOCOL_FEES_PERCENTAGE: u8 = 10;

/// The discriminator for the external undelegate instruction.
pub const EXTERNAL_UNDELEGATE_DISCRIMINATOR: [u8; 8] = [196, 28, 41, 206, 48, 37, 51, 167];

/// The discriminator for the external hook after finalization is complete
/// For anchor: corresponds to function/instruction name delegation_program_call_handler
pub const EXTERNAL_CALL_HANDLER_DISCRIMINATOR: [u8; 8] = [157, 197, 228, 30, 0, 80, 121, 135];

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
pub const BROADCAST_IDENTITY: Pubkey = pubkey!("Broadcast1111111111111111111111111111111111");
