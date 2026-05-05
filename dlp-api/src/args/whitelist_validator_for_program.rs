use borsh_0_10::{BorshDeserialize, BorshSerialize};

#[derive(Debug, BorshSerialize, BorshDeserialize)]
#[borsh(crate = "borsh_0_10")]
pub struct WhitelistValidatorForProgramArgs {
    /// If `true`, insert the validator identity into the program whitelist,
    /// otherwise remove it.
    pub insert: bool,
}
