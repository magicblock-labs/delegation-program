use borsh::{BorshDeserialize, BorshSerialize};

#[derive(Default, Debug, BorshSerialize, BorshDeserialize)]
pub struct ValidatorClaimFeesArgs {
    /// The amount to claim from the fees vault.
    /// If `None`, almost the entire amount is claimed. The remaining amount
    /// is needed to keep the fees vault rent-exempt.
    pub amount: Option<u64>,
}
