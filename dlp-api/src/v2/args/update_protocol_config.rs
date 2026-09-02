// Update uses the same payload as init because every init-time config value in
// this args type is also authority-updatable. Account identity fields such as
// authority, fee vault, pause state, discriminator, and bump are preserved by
// the processor instead of coming from instruction data.
pub type UpdateProtocolConfigArgs = super::InitProtocolConfigArgs;
