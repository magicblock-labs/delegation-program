use wheels::variable_offset_layout;

#[derive(Clone, Debug, PartialEq, Eq)]
#[variable_offset_layout(buffer_offset = 1)]
pub struct RegisterVerifierArgs {
    pub stake_lamports: u64,
}
