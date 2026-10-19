//! Processors for v2 fraud-proof instructions.

mod propose_commitment;
mod write_state_buffer;

pub use propose_commitment::*;
pub use write_state_buffer::*;
