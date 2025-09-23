use crate::{impl_to_bytes_with_discriminator_light, impl_try_from_bytes_with_discriminator_light};
use borsh::{BorshDeserialize, BorshSerialize};
use light_hasher::{DataHasher, Hasher, HasherError, Sha256};
use light_sdk::LightDiscriminator;
use solana_program::pubkey::Pubkey;

/// The Delegated Metadata includes Account Seeds, max delegation time, seeds
/// and other meta information about the delegated account.
/// * Everything necessary at cloning time is instead stored in the delegation record.
#[derive(BorshSerialize, BorshDeserialize, Debug, Default, PartialEq)]
pub struct DelegatedCompressedAccount {
    /// The original account data as a utf-8 string
    pub account_data: Vec<u8>,
    /// The seeds of the account, used to reopen it on undelegation
    pub seeds: Vec<Vec<u8>>,
    /// The delegated authority
    pub authority: Pubkey,
    /// The original owner of the account
    pub owner: Pubkey,
    /// The slot at which the delegation was created
    pub delegation_slot: u64,
    /// The state update frequency in milliseconds
    pub commit_frequency_ms: u64,
}

impl DataHasher for DelegatedCompressedAccount {
    fn hash<H: Hasher>(&self) -> Result<[u8; 32], HasherError> {
        let bytes = self.try_to_vec().unwrap();
        let mut hash = Sha256::hash(&bytes.as_slice())?;
        hash[0] = 0;
        Ok(hash)
    }
}

impl LightDiscriminator for DelegatedCompressedAccount {
    const LIGHT_DISCRIMINATOR: [u8; 8] = [104, 0, 0, 0, 0, 0, 0, 0];
    const LIGHT_DISCRIMINATOR_SLICE: &'static [u8] = &[104, 0, 0, 0, 0, 0, 0, 0];
}

impl_to_bytes_with_discriminator_light!(DelegatedCompressedAccount);
impl_try_from_bytes_with_discriminator_light!(DelegatedCompressedAccount);

#[cfg(test)]
mod tests {
    use borsh::to_vec;

    use super::*;

    #[derive(BorshSerialize, BorshDeserialize)]
    struct TestAccount {
        pub value: u64,
    }

    #[test]
    fn test_serialization_without_discriminator() {
        let original = DelegatedCompressedAccount {
            account_data: TestAccount { value: 1 }.try_to_vec().unwrap(),
            seeds: vec![
                vec![],
                vec![
                    215, 233, 74, 188, 162, 203, 12, 212, 106, 87, 189, 226, 48, 38, 129, 7, 34,
                    82, 254, 106, 161, 35, 74, 146, 30, 211, 164, 97, 139, 136, 136, 77,
                ],
            ],
            authority: Pubkey::default(),
            owner: Pubkey::default(),
            delegation_slot: 0,
            commit_frequency_ms: 0,
        };

        // Serialize
        let serialized = to_vec(&original).expect("Serialization failed");

        // Deserialize
        let deserialized = DelegatedCompressedAccount::try_from_slice(&serialized)
            .expect("Deserialization failed");

        assert_eq!(deserialized, original);
    }
}
