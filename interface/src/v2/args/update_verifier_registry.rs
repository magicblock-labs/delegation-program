use wheels::variable_offset_layout;

#[derive(Clone, Debug, PartialEq, Eq)]
#[variable_offset_layout(buffer_offset = 1)]
pub struct UpdateVerifierRegistryArgs {
    pub action: u8,
    pub weight: u64,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerifierRegistryAction {
    Add = 1,
    Remove = 2,
}

impl VerifierRegistryAction {
    pub const fn value(self) -> u8 {
        self as u8
    }
}
