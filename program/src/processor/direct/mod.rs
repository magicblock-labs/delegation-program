//! Processors for clusters that finalize without fraud proofs.

mod commit_finalize;
mod commit_finalize_internal;

pub use commit_finalize::*;
pub(crate) use commit_finalize_internal::*;
