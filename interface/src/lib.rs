pub use ::solana_program;

pub mod args;
pub mod compact;
pub mod compat;
pub mod consts;

#[cfg(feature = "diff")]
pub mod diff;

pub mod error;
pub mod instruction;
pub mod legacy;
pub mod pda;
pub mod requires;
pub mod state;

#[cfg(feature = "instruction")]
pub mod instruction_builder;

pub use args::*;
pub use consts::*;
pub use instruction::*;
pub use pda::*;
pub use state::*;

#[cfg(feature = "encryption")]
pub mod decrypt;

#[cfg(feature = "encryption")]
pub mod encrypt;

#[cfg(feature = "encryption")]
pub mod encryption;

#[cfg(feature = "encryption")]
pub use decrypt::*;

compat::declare_id!("DELeGGvXpWV2fqJUhqcF5ZSYMS4JTLjteaAMARRSaeSh");

pub mod fast {
    pinocchio::address::declare_id!(
        "DELeGGvXpWV2fqJUhqcF5ZSYMS4JTLjteaAMARRSaeSh"
    );
}
