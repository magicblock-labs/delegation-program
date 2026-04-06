pub use ::solana_program::{declare_id, *};

pub mod system_instruction {
    pub use solana_system_interface::instruction::*;
}

pub mod system_program {
    pub use solana_system_interface::program::{id, ID};
}

pub mod bpf_loader_upgradeable {
    pub use solana_loader_v3_interface::state::UpgradeableLoaderState;
    pub use solana_sdk_ids::bpf_loader_upgradeable::{id, ID};
}
