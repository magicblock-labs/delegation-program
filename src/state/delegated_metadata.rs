use crate::state::utils::account::{AccountDiscriminator, Discriminator};
use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::pubkey::Pubkey;

/// The Delegated Metadata includes Account Seeds, max delegation time, seeds
/// and other meta information about the delegated account.
/// * Everything necessary at cloning time is instead stored in the delegation record.
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct DelegationMetadata {
    /// The delegation validity
    pub valid_until: i64,
    /// The last slot at which the delegation was updated
    pub last_update_external_slot: u64,
    /// Whether the account can be undelegated or not
    pub is_undelegatable: bool,
    /// The seeds of the account, used to reopen it on undelegation
    pub seeds: Vec<Vec<u8>>,
    /// The account that paid the rent for the delegation PDAs
    pub rent_payer: Pubkey,
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
            rent_payer: Pubkey::default(),
        };

        // Serialize
        let serialized = seeds.try_to_vec().expect("Serialization failed");

        // Deserialize
        let deserialized: DelegationMetadata =
            DelegationMetadata::try_from_slice(&serialized).expect("Deserialization failed");

        assert_eq!(deserialized.seeds, seeds.seeds);
    }
}
