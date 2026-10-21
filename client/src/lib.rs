pub use ::solana_program;

#[cfg(feature = "diff")]
pub mod diff;

#[cfg(feature = "encryption")]
pub mod encryption;

#[cfg(feature = "encryption")]
pub mod instruction_builder;

#[cfg(feature = "encryption")]
pub use encryption::{Decrypt, DecryptError};
