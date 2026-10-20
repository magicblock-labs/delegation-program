// V2 processors decode args from `instruction_data[1..]` after the one-byte
// instruction tag, so v2 instruction args use `buffer_offset = 1`.

mod init_delegation_cluster;
mod register_operator;
mod register_verifier;
mod update_verifier_registry;

pub use init_delegation_cluster::*;
pub use register_operator::*;
pub use register_verifier::*;
pub use update_verifier_registry::*;
