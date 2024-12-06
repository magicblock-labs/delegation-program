use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::pubkey::Pubkey;
use std::collections::BTreeSet;

use crate::{consts::PROGRAM_CONFIG_DISCRIMINANT, impl_try_from_bytes_with_discriminant_borsh};

#[derive(BorshSerialize, BorshDeserialize, Default, Debug)]
pub struct ProgramConfig {
    pub approved_validators: BTreeSet<Pubkey>,
}

impl ProgramConfig {
    pub fn discriminant() -> &'static [u8; 8] {
        return PROGRAM_CONFIG_DISCRIMINANT;
    }
    pub fn size_with_discriminant(&self) -> usize {
        return 8 + 4 + 32 * self.approved_validators.len();
    }
}

impl_try_from_bytes_with_discriminant_borsh(ProgramConfig);
