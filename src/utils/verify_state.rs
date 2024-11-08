/// Add the logic to check the state diff, Authority and/or Fraud proofs
use crate::state::{CommitRecord, DelegationRecord};
use solana_program::account_info::AccountInfo;
use solana_program::entrypoint::ProgramResult;

/// Verify the committed state
#[inline(always)]
pub(crate) fn verify_state(
    _authority: &AccountInfo,
    _delegation_record: &DelegationRecord,
    _committed_record: &CommitRecord,
    _committed_state: &AccountInfo,
) -> ProgramResult {
    // TODO: Temporary relying on the assumption than the validator fees vault exists (as it was created by the admin)
    Ok(())
}
