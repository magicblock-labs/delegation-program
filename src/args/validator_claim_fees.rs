use borsh::{BorshDeserialize, BorshSerialize};

#[derive(Default, Debug, BorshSerialize, BorshDeserialize)]
pub struct ValidatorClaimFeesArgs {
    pub amount: Option<u64>,
}
