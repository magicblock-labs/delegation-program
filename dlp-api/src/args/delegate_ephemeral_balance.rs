use borsh_0_10::{BorshDeserialize, BorshSerialize};

use crate::args::DelegateArgs;

#[derive(Default, Debug, BorshSerialize, BorshDeserialize)]
#[borsh(crate = "borsh_0_10")]
pub struct DelegateEphemeralBalanceArgs {
    pub delegate_args: DelegateArgs,
    pub index: u8,
}
