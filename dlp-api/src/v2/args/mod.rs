// V2 processors decode args from `instruction_data[1..]` after the one-byte
// instruction tag, so v2 instruction args use `buffer_offset = 1`.

mod init_protocol_config;
mod register_operator;
mod register_verifier;

pub use init_protocol_config::*;
pub use register_operator::*;
pub use register_verifier::*;
