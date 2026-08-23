use wheels::variable_offset_layout;

pub const VERIFIER_REGISTRY_ACTION_ADD: u8 = 1;
pub const VERIFIER_REGISTRY_ACTION_REMOVE: u8 = 2;

#[derive(Clone, Debug, PartialEq, Eq)]
#[variable_offset_layout(buffer_offset = 0)]
pub struct UpdateVerifierRegistryArgs {
    pub action: u8,
    pub weight: u64,
}
