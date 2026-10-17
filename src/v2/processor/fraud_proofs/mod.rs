//! Processors for v2 fraud-proof instructions.

mod approve_commitment;
mod finalize_commitment;
mod post_commitment;
mod raise_challenge;
mod write_state_buffer;

pub use approve_commitment::*;
pub use finalize_commitment::*;
pub use post_commitment::*;
pub use raise_challenge::*;
pub use write_state_buffer::*;
