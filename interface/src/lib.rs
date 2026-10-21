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
pub mod v2;

pub use account_size_class::*;

#[cfg(feature = "cpi")]
pub mod cpi;

#[cfg(feature = "instruction")]
pub mod instruction_builder;

compat::declare_id!("DELeGGvXpWV2fqJUhqcF5ZSYMS4JTLjteaAMARRSaeSh");

pub mod fast {
    pinocchio::address::declare_id!(
        "DELeGGvXpWV2fqJUhqcF5ZSYMS4JTLjteaAMARRSaeSh"
    );
}
