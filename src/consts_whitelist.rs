/// Whitelisted identities
/// TODO: Hardcoded for now, should use a dynamic list
use solana_program::pubkey;
use solana_program::pubkey::Pubkey;

use std::collections::HashMap;

#[cfg(not(mainnet))]
const WHITELISTED_IDENTITIES: &[(Pubkey, Vec<Pubkey>)] = &[
    (
        pubkey!("mAGicPQYBMvcYveUZA5F5UNNwyHvfYh5xkLS2Fr1mev"),
        vec![],
    ),
    (
        pubkey!("zbitnhqG6MLu3E6XBJGEd7WarnKDeqzriB14hr74Fjb"),
        vec![],
    ),
    (
        pubkey!("sups7xRrKcWsVoGEsuoYp7o4dAdwDPEMpHH1sxYKEm4"),
        vec![],
    ),
    (
        pubkey!("tEsT3eV6RFCWs1BZ7AXTzasHqTtMnMLCB2tjQ42TDXD"),
        vec![],
    ),
];

#[cfg(mainnet)]
const WHITELISTED_IDENTITIES: &[(Pubkey, Vec<Pubkey>)] = &[
    (
        pubkey!("zbitnhqG6MLu3E6XBJGEd7WarnKDeqzriB14hr74Fjb"),
        vec![],
    ),
    (
        pubkey!("sups7xRrKcWsVoGEsuoYp7o4dAdwDPEMpHH1sxYKEm4"),
        vec![],
    ),
];

/// Get the whitelisted identities
pub fn get_whitelisted_identities() -> HashMap<Pubkey, Vec<Pubkey>> {
    WHITELISTED_IDENTITIES.iter().cloned().collect()
}
