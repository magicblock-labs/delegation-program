use crate::utils_account::{AccountDiscriminator, Discriminator};
use borsh::{BorshDeserialize, BorshSerialize};

/// The Delegated Account Seeds
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct DelegateAccountSeeds {
    pub seeds: Vec<Vec<u8>>,
}

impl Discriminator for DelegateAccountSeeds {
    fn discriminator() -> AccountDiscriminator {
        AccountDiscriminator::DelegationRecord
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialization() {
        let seeds = DelegateAccountSeeds {
            seeds: vec![
                vec![],
                vec![
                    215, 233, 74, 188, 162, 203, 12, 212, 106, 87, 189, 226, 48, 38, 129, 7, 34,
                    82, 254, 106, 161, 35, 74, 146, 30, 211, 164, 97, 139, 136, 136, 77,
                ],
            ],
        };

        // Serialize
        let serialized = seeds.try_to_vec().expect("Serialization failed");

        // Deserialize
        let deserialized: DelegateAccountSeeds =
            DelegateAccountSeeds::try_from_slice(&serialized).expect("Deserialization failed");

        assert_eq!(deserialized.seeds, seeds.seeds);
    }
}
