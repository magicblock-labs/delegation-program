extern crate self as dlp;
pub use ::solana_program;

pub mod compat;

pub mod account_size_class;
pub mod args;
pub mod compact;
pub mod consts;

#[cfg(feature = "diff")]
pub mod diff;

pub mod discriminator;
pub mod error;
pub mod pda;
pub mod pod_view;
pub mod requires;
pub mod state;

pub use account_size_class::*;
pub use wheels;

#[cfg(feature = "cpi")]
pub mod cpi;

#[cfg(feature = "instruction")]
pub mod instruction_builder;

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
