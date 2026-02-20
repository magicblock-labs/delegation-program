use std::ptr;

use borsh::{BorshDeserialize, BorshSerialize};
use pinocchio::{account::RefMut, error::ProgramError, AccountView};
use solana_program::pubkey::Pubkey;

use super::discriminator::{AccountDiscriminator, AccountWithDiscriminator};
use crate::{
    impl_to_bytes_with_discriminator_borsh,
    impl_try_from_bytes_with_discriminator_borsh, require_ge,
};

/// The Delegated Metadata includes Account Seeds, max delegation time, seeds
/// and other meta information about the delegated account.
/// * Everything necessary at cloning time is instead stored in the delegation record.
#[derive(BorshSerialize, BorshDeserialize, Debug, PartialEq)]
pub struct DelegationMetadata {
    /// The last nonce account had during delegation update
    /// Deprecated: The last slot at which the delegation was updated
    pub last_update_nonce: u64,
    /// Whether the account can be undelegated or not
    pub is_undelegatable: bool,
    /// The seeds of the account, used to reopen it on undelegation
    pub seeds: Vec<Vec<u8>>,
    /// The account that paid the rent for the delegation PDAs
    pub rent_payer: Pubkey,
}

pub struct DelegationMetadataFast<'a> {
    data: RefMut<'a, [u8]>,
}

impl<'a> DelegationMetadataFast<'a> {
    pub fn from_account(
        account: &'a AccountView,
    ) -> Result<Self, ProgramError> {
        require_ge!(
            account.data_len(),
            AccountDiscriminator::SPACE
            + 8  // last_update_nonce
            + 1  // is_undelegatable
            + 32 // rent_payer
            + 4, // seeds (at least 4)
            ProgramError::InvalidAccountData
        );

        Ok(Self {
            data: account.try_borrow_mut()?,
        })
    }

    pub fn last_update_nonce(&self) -> u64 {
        unsafe {
            ptr::read(self.data.as_ptr().add(AccountDiscriminator::SPACE)
                as *const u64)
        }
    }

    pub fn set_last_update_nonce(&mut self, val: u64) {
        unsafe {
            ptr::write(
                self.data.as_mut_ptr().add(AccountDiscriminator::SPACE)
                    as *mut u64,
                val,
            )
        }
    }

    pub fn replace_last_update_nonce(&mut self, val: u64) -> u64 {
        unsafe {
            ptr::replace(
                self.data.as_mut_ptr().add(AccountDiscriminator::SPACE)
                    as *mut u64,
                val,
            )
        }
    }

    pub fn set_is_undelegatable(&mut self, val: bool) {
        unsafe {
            ptr::write(
                self.data.as_mut_ptr().add(AccountDiscriminator::SPACE + 8)
                    as *mut bool,
                val,
            )
        }
    }

    pub fn replace_is_undelegatable(&mut self, val: bool) -> bool {
        unsafe {
            ptr::replace(
                self.data.as_mut_ptr().add(AccountDiscriminator::SPACE + 8)
                    as *mut bool,
                val,
            )
        }
    }
}

impl AccountWithDiscriminator for DelegationMetadata {
    fn discriminator() -> AccountDiscriminator {
        AccountDiscriminator::DelegationMetadata
    }
}

impl DelegationMetadata {
    pub fn serialized_size(&self) -> usize {
        AccountDiscriminator::SPACE
        + 8 // last_update_nonce (u64) 
        + 1 // is_undelegatable (bool)
        + 32 // rent_payer (Pubkey)
        + (4 + self.seeds.iter().map(|s| 4 + s.len()).sum::<usize>()) // seeds (Vec<Vec<u8>>)
    }
}

impl_to_bytes_with_discriminator_borsh!(DelegationMetadata);
impl_try_from_bytes_with_discriminator_borsh!(DelegationMetadata);

#[cfg(test)]
mod tests {
    use borsh::to_vec;

    use super::*;

    #[test]
    fn test_serialization_without_discriminator() {
        let original = DelegationMetadata {
            seeds: vec![
                vec![],
                vec![
                    215, 233, 74, 188, 162, 203, 12, 212, 106, 87, 189, 226,
                    48, 38, 129, 7, 34, 82, 254, 106, 161, 35, 74, 146, 30,
                    211, 164, 97, 139, 136, 136, 77,
                ],
            ],
            is_undelegatable: false,
            last_update_nonce: 0,
            rent_payer: Pubkey::default(),
        };

        // Serialize
        let serialized = to_vec(&original).expect("Serialization failed");

        // Deserialize
        let deserialized: DelegationMetadata =
            DelegationMetadata::try_from_slice(&serialized)
                .expect("Deserialization failed");

        assert_eq!(deserialized, original);
    }
}
