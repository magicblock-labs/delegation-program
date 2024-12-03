use solana_curve25519::edwards::{validate_edwards, PodEdwardsPoint};
use solana_program::account_info::AccountInfo;

/// Define a trait to add is_on_curve method to AccountInfo
pub trait ValidateEdwards {
    fn is_on_curve(&self) -> bool;
}

/// Implement the trait for AccountInfo
impl ValidateEdwards for AccountInfo<'_> {
    fn is_on_curve(&self) -> bool {
        validate_edwards(&PodEdwardsPoint(self.key.to_bytes()))
    }
}
