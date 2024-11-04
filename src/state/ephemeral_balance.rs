use bytemuck::{Pod, Zeroable};

use crate::utils::utils_account::{AccountDiscriminator, Discriminator};
use crate::{impl_account_from_bytes, impl_to_bytes};

/// The Ephemeral Balance
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct EphemeralBalance {
    /// The deposited lamports
    pub lamports: u64,
}

impl Discriminator for EphemeralBalance {
    fn discriminator() -> AccountDiscriminator {
        AccountDiscriminator::EphemeralBalance
    }
}

impl_to_bytes!(EphemeralBalance);
impl_account_from_bytes!(EphemeralBalance);
