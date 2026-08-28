use wheels::variable_offset_layout;

pub const DISPUTE_DECISION_OPERATOR_STATE_CORRECT: u8 = 1;
pub const DISPUTE_DECISION_CHALLENGER_STATE_CORRECT: u8 = 2;

#[derive(Clone, Debug, PartialEq, Eq)]
#[variable_offset_layout(buffer_offset = 1)]
pub struct ResolveDisputeArgs {
    /// Resolver decision for a valid mismatched reveal.
    pub decision: u8,
}
