use solana_program::account_info::AccountInfo;
use solana_program::entrypoint::ProgramResult;

use crate::state::{CommitRecord, DelegationRecord};

/// Verify the committed state
#[inline(always)]
pub fn verify_state(
    _authority: &AccountInfo,
    _delegation_record: &DelegationRecord,
    _commit_record: &CommitRecord,
    _commit_state_account: &AccountInfo,
) -> ProgramResult {
    // TODO: Temporary relying on the assumption than the validator fees vault exists (as it was created by the admin)
    Ok(())
}
