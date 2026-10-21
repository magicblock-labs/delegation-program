use borsh::{BorshDeserialize, BorshSerialize};

use crate::{compat::borsh, legacy::args::DelegateArgs};

#[derive(Default, Debug, BorshSerialize, BorshDeserialize)]
pub struct DelegateEphemeralBalanceArgs {
    pub delegate_args: DelegateArgs,
    pub index: u8,
}
