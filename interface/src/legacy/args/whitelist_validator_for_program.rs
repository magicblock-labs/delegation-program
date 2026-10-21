use borsh::{BorshDeserialize, BorshSerialize};

use crate::compat::borsh;

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct WhitelistValidatorForProgramArgs {
    /// If `true`, insert the validator identity into the program whitelist,
    /// otherwise remove it.
    pub insert: bool,
}
