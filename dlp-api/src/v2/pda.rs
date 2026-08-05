use crate::compat::Pubkey;

pub const PROTOCOL_CONFIG_SEED: &[u8] = b"protocol-config";
pub const VERIFIER_REGISTRY_SEED: &[u8] = b"verifier-registry";

pub fn protocol_config_pda() -> Pubkey {
    Pubkey::find_program_address(&[PROTOCOL_CONFIG_SEED], &crate::id()).0
}

pub fn verifier_registry_pda() -> Pubkey {
    Pubkey::find_program_address(&[VERIFIER_REGISTRY_SEED], &crate::id()).0
}
