use borsh_0_10::{BorshDeserialize, BorshSerialize};

#[derive(Default, Debug, BorshSerialize, BorshDeserialize)]
#[borsh(crate = "borsh_0_10")]
pub struct ValidatorClaimFeesArgs {
    /// The amount to claim from the fees vault.
    /// If `None`, almost the entire amount is claimed. The remaining amount
    /// is needed to keep the fees vault rent-exempt.
    pub amount: Option<u64>,
}
