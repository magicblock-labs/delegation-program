use crate::utils_account::{AccountDiscriminator, Discriminator};
use borsh::{BorshDeserialize, BorshSerialize};

/// The Delegated Metadata includes Account Seeds, max delegation time, seeds
/// and other meta information about the delegated account.
/// * Everything necessary at cloning time is instead stored in the delegation record.
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct DelegationMetadata {
    /// The delegation validity
    pub valid_until: i64,
    pub last_update_external_slot: u64,
    pub is_undelegatable: bool,
    pub seeds: Vec<Vec<u8>>,
}

impl Discriminator for DelegationMetadata {
    fn discriminator() -> AccountDiscriminator {
        AccountDiscriminator::DelegatedMetadata
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialization() {
        let seeds = DelegationMetadata {
            seeds: vec![
                vec![],
                vec![
                    215, 233, 74, 188, 162, 203, 12, 212, 106, 87, 189, 226, 48, 38, 129, 7, 34,
                    82, 254, 106, 161, 35, 74, 146, 30, 211, 164, 97, 139, 136, 136, 77,
                ],
            ],
            is_undelegatable: false,
            last_update_external_slot: 0,
            valid_until: 0,
        };

        // Serialize
        let serialized = seeds.try_to_vec().expect("Serialization failed");

        // Deserialize
        let deserialized: DelegationMetadata =
            DelegationMetadata::try_from_slice(&serialized).expect("Deserialization failed");

        assert_eq!(deserialized.seeds, seeds.seeds);
    }
}
