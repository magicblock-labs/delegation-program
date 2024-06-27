use solana_program::pubkey;
use solana_program::pubkey::Pubkey;

/// The seed of the buffer account PDA.
pub const BUFFER: &[u8] = b"buffer";

/// The delegation program ID.
pub const DELEGATION_PROGRAM_ID: Pubkey = pubkey!("DELeGGvXpWV2fqJUhqcF5ZSYMS4JTLjteaAMARRSaeSh");

/// The magic program ID.
pub const MAGIC_PROGRAM_ID: Pubkey = pubkey!("Magic11111111111111111111111111111111111111");
