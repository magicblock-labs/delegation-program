use crate::state::{CommitRecord, DelegationRecord};
use solana_program::account_info::AccountInfo;
use solana_program::entrypoint::ProgramResult;

/// Verify the committed state
#[inline(always)]
pub(crate) fn verify_state(
    _delegation_record: &DelegationRecord,
    _committed_state: &CommitRecord,
    _new_state: &AccountInfo,
) -> ProgramResult {
    // TODO: Add the logic to check the state diff, Authority and/or Fraud proofs
    Ok(())
}
