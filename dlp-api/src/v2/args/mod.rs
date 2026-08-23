// V2 processors decode args from `instruction_data[1..]` after the one-byte
// instruction tag, so v2 instruction args use `buffer_offset = 1`.

mod init_protocol_config;
mod post_commitment;
mod register_operator;
mod register_verifier;
mod update_protocol_config;
mod update_verifier_registry;
mod write_state_buffer;

pub use init_protocol_config::*;
pub use post_commitment::*;
pub use register_operator::*;
pub use register_verifier::*;
pub use update_protocol_config::*;
pub use update_verifier_registry::*;
pub use write_state_buffer::*;
