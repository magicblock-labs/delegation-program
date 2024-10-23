use solana_program::account_info::AccountInfo;
use solana_program::entrypoint::ProgramResult;

use crate::state::{CommitRecord, DelegationRecord};
use crate::verify_state::verify_state;

/// Verify the committed state
#[inline(always)]
pub(crate) fn verify_commitment(
    authority: &AccountInfo,
    delegation_record: &DelegationRecord,
    committed_record: &CommitRecord,
    committed_state: &AccountInfo,
) -> ProgramResult {
    verify_state(
        authority,
        delegation_record,
        committed_record,
        committed_state,
    )
}
