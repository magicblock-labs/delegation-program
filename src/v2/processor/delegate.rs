use pinocchio::{AccountView, ProgramResult};

use crate::v2::processor::internal::process_delegate_internal;

pub fn process_delegate(
    accounts: &[AccountView],
    data: &[u8],
) -> ProgramResult {
    process_delegate_internal::<false>(accounts, data)
}

///
/// delegates an account while allowing any validator identity
///
pub fn process_delegate_with_any_validator(
    accounts: &[AccountView],
    data: &[u8],
) -> ProgramResult {
    process_delegate_internal::<true>(accounts, data)
}
