// TODO: Temporary whitelist check. Add the logic to check the state diff, Authority and/or Fraud proofs

use crate::consts_whitelist::get_whitelisted_identities;
use crate::error::DlpError;
use crate::state::{CommitRecord, DelegationRecord};
use solana_program::account_info::AccountInfo;
use solana_program::entrypoint::ProgramResult;
use solana_program::pubkey::Pubkey;

/// Verify the committed state
#[inline(always)]
pub(crate) fn verify_state(
    authority: &AccountInfo,
    delegation_record: &DelegationRecord,
    _committed_record: &CommitRecord,
    _committed_state: &AccountInfo,
) -> ProgramResult {
    let whitelisted_identities = get_whitelisted_identities();
    let allowed_programs: &Vec<Pubkey> = whitelisted_identities
        .get(authority.key)
        .ok_or(DlpError::InvalidAuthority)?;
    if !allowed_programs.is_empty() && !allowed_programs.contains(&delegation_record.owner) {
        return Err(DlpError::InvalidAuthorityForProgram.into());
    }
    Ok(())
}
