//! Processors for v2 fraud-proof instructions.

mod approve_commitment;
mod post_commitment;
mod write_state_buffer;

pub use approve_commitment::*;
pub use post_commitment::*;
pub use write_state_buffer::*;
