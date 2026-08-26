//! Processors for v2 fraud-proof instructions.

mod approve_commitment;
mod finalize_commitment;
mod propose_commitment;
mod write_state_buffer;

pub use approve_commitment::*;
pub use finalize_commitment::*;
pub use propose_commitment::*;
pub use write_state_buffer::*;
