use solana_curve25519::edwards::{validate_edwards, PodEdwardsPoint};
use solana_program::pubkey::Pubkey;

pub fn is_on_curve(key: &Pubkey) -> bool {
    validate_edwards(&PodEdwardsPoint(key.to_bytes()))
}
