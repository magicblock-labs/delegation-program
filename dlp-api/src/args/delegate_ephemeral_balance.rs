use borsh::{BorshDeserialize, BorshSerialize};

use crate::{args::DelegateArgs, compat::borsh};

#[derive(Default, Debug, BorshSerialize, BorshDeserialize)]
pub struct DelegateEphemeralBalanceArgs {
    pub delegate_args: DelegateArgs,
    pub index: u8,
}
