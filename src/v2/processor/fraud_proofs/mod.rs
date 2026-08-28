//! Processors for v2 fraud-proof instructions.

mod approve_commitment;
mod challenger_reveal;
mod finalize_commitment;
mod post_commitment;
mod raise_challenge;
mod resolve_dispute;
mod write_state_buffer;

pub use approve_commitment::*;
pub use challenger_reveal::*;
pub use finalize_commitment::*;
pub use post_commitment::*;
pub use raise_challenge::*;
pub use resolve_dispute::*;
pub use write_state_buffer::*;
