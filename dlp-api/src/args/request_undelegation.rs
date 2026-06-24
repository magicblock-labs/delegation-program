use wheels::variable_offset_layout;

#[variable_offset_layout(buffer_offset = 0)]
pub struct RequestUndelegationArgs {
    pub timeout_slots: Option<u16>, // number of slots as timeout
}
