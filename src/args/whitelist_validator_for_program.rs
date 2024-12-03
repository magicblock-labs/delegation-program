use borsh::{BorshDeserialize, BorshSerialize};

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct WhitelistValidatorForProgramArgs {
    pub insert: bool,
}
