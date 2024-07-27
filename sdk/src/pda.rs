// NOTE: this should go into a core package that both the sdk + the program can depend on
use paste::paste;

use crate::consts::{BUFFER, COMMIT_RECORD, COMMIT_STATE, DELEGATION_METADATA, DELEGATION_RECORD};

// -----------------
// Seeds
// -----------------
macro_rules! seeds {
    ($prefix:ident, $bytes_const:expr) => {
        paste! {
            #[allow(clippy::needless_lifetimes)]
            pub fn [<$prefix _seeds>]<'a>(pda_id: &'a [u8]) -> [&'a [u8]; 2] {
                [$bytes_const, pda_id]
            }
            #[allow(clippy::needless_lifetimes)]
            pub fn [<$prefix _seeds_with_bump>]<'a>(pda_id: &'a [u8], bump: &'a [u8; 1]) -> [&'a [u8]; 3] {
                [$bytes_const, pda_id, bump]
            }
            #[allow(clippy::needless_lifetimes)]
            pub fn [<$prefix _seeds_from_pubkey>]<'a>(pda_id: &'a ::solana_program::pubkey::Pubkey) -> [&'a [u8]; 2] {
                [$bytes_const, pda_id.as_ref()]
            }
            #[allow(clippy::needless_lifetimes)]
            pub fn [<$prefix _seeds_with_bump_from_pubkey>]<'a>(
                pda_id: &'a ::solana_program::pubkey::Pubkey,
                bump: &'a [u8; 1],
            ) -> [&'a [u8]; 3] {
                [$bytes_const, pda_id.as_ref(), bump]
            }
        }
    };
}

// -----------------
// PDA
// -----------------
macro_rules! pda {
    ($prefix:ident) => {
        paste! {
            #[allow(clippy::needless_lifetimes)]
            pub fn [<$prefix _pda_with_bump>]<'a>(pda_id: &'a [u8]) -> (::solana_program::pubkey::Pubkey, u8) {
                let seeds = [<$prefix _seeds>](pda_id);
                ::solana_program::pubkey::Pubkey::find_program_address(
                    &seeds,
                    &crate::id()
                )
            }
            #[allow(clippy::needless_lifetimes)]
            pub fn [<$prefix _pda>]<'a>(pda_id: &'a [u8]) -> ::solana_program::pubkey::Pubkey {
                [<$prefix _pda_with_bump>](pda_id).0
            }
            #[allow(clippy::needless_lifetimes)]
            pub fn [<$prefix _pda_with_bump_from_pubkey>]<'a>(pda_id: &'a ::solana_program::pubkey::Pubkey) -> (::solana_program::pubkey::Pubkey, u8) {
                let seeds = [<$prefix _seeds_from_pubkey>](pda_id);
                ::solana_program::pubkey::Pubkey::find_program_address(
                    &seeds,
                    &crate::id()
                )
            }
            #[allow(clippy::needless_lifetimes)]
            pub fn [<$prefix _pda_from_pubkey>]<'a>(pda_id: &'a ::solana_program::pubkey::Pubkey) -> ::solana_program::pubkey::Pubkey {
                [<$prefix _pda_with_bump_from_pubkey>](pda_id).0
            }
        }
    };
}

seeds! { delegation_record, DELEGATION_RECORD }
pda! { delegation_record }

seeds! { delegation_metadata, DELEGATION_METADATA }
pda! { delegation_metadata }

seeds! { committed_state, COMMIT_STATE }
pda! { committed_state }

seeds! { committed_state_record, COMMIT_RECORD }
pda! { committed_state_record }

seeds! { buffer, BUFFER }
pda! { buffer }

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use solana_program::pubkey::Pubkey;

    use crate::consts::{COMMIT_RECORD, COMMIT_STATE, DELEGATION_PROGRAM_ID, DELEGATION_RECORD};

    use super::*;

    // -----------------
    // Delegation Seeds
    // -----------------
    #[test]
    fn test_delegation_record_seeds() {
        let id = [1, 2, 3];
        let seeds = delegation_record_seeds(&id);
        assert_eq!(seeds, [DELEGATION_RECORD, &id]);
    }

    #[test]
    fn test_delegation_record_seeds_with_bump() {
        let id = [1, 2, 3];
        let bump = [4];
        let seeds = delegation_record_seeds_with_bump(&id, &bump);
        assert_eq!(seeds, [DELEGATION_RECORD, &id, &bump]);
    }

    #[test]
    fn test_delegation_record_seeds_from_pubkey() {
        let id = Pubkey::new_unique();
        let seeds = delegation_record_seeds_from_pubkey(&id);
        assert_eq!(seeds, [DELEGATION_RECORD, id.as_ref()]);
    }

    #[test]
    fn test_delegation_record_seeds_with_bump_from_pubkey() {
        let id = Pubkey::new_unique();
        let bump = [4];
        let seeds = delegation_record_seeds_with_bump_from_pubkey(&id, &bump);
        assert_eq!(seeds, [DELEGATION_RECORD, id.as_ref(), &bump]);
    }

    // -----------------
    // State Diff Seeds
    // -----------------
    #[test]
    fn test_committed_state_seeds() {
        let id = [1, 2, 3];
        let seeds = committed_state_seeds(&id);
        assert_eq!(seeds, [COMMIT_STATE, &id]);
    }

    #[test]
    fn test_committed_state_seeds_with_bump() {
        let id = [1, 2, 3];
        let bump = [4];
        let seeds = committed_state_seeds_with_bump(&id, &bump);
        assert_eq!(seeds, [COMMIT_STATE, &id, &bump]);
    }

    #[test]
    fn test_committed_state_seeds_from_pubkey() {
        let id = Pubkey::new_unique();
        let seeds = committed_state_seeds_from_pubkey(&id);
        assert_eq!(seeds, [COMMIT_STATE, id.as_ref()]);
    }

    #[test]
    fn test_committed_state_seeds_with_bump_from_pubkey() {
        let id = Pubkey::new_unique();
        let bump = [4];
        let seeds = committed_state_seeds_with_bump_from_pubkey(&id, &bump);
        assert_eq!(seeds, [COMMIT_STATE, id.as_ref(), &bump]);
    }

    // -----------------
    // Commit Record Seeds
    // -----------------
    #[test]
    fn test_commit_record_seeds() {
        let id = [1, 2, 3];
        let seeds = committed_state_record_seeds(&id);
        assert_eq!(seeds, [COMMIT_RECORD, &id]);
    }

    #[test]
    fn test_commit_record_seeds_with_bump() {
        let id = [1, 2, 3];
        let bump = [4];
        let seeds = committed_state_record_seeds_with_bump(&id, &bump);
        assert_eq!(seeds, [COMMIT_RECORD, &id, &bump]);
    }

    #[test]
    fn test_commit_record_seeds_from_pubkey() {
        let id = Pubkey::new_unique();
        let seeds = committed_state_record_seeds_from_pubkey(&id);
        assert_eq!(seeds, [COMMIT_RECORD, id.as_ref()]);
    }

    #[test]
    fn test_commit_record_seeds_with_bump_from_pubkey() {
        let id = Pubkey::new_unique();
        let bump = [4];
        let seeds = committed_state_record_seeds_with_bump_from_pubkey(&id, &bump);
        assert_eq!(seeds, [COMMIT_RECORD, id.as_ref(), &bump]);
    }

    // -----------------
    // Delegation PDA
    // -----------------
    #[test]
    fn test_delegation_record_pda() {
        let id = Pubkey::new_unique();
        let pda = delegation_record_pda(id.as_ref());
        let seeds = delegation_record_seeds(id.as_ref());
        let expected = Pubkey::find_program_address(&seeds, &DELEGATION_PROGRAM_ID).0;
        assert_eq!(pda, expected);
    }

    #[test]
    fn test_delegation_record_pda_with_bump() {
        let id = Pubkey::new_unique();
        let (pda, bump) = delegation_record_pda_with_bump(id.as_ref());
        let seeds = delegation_record_seeds(id.as_ref());
        let expected = Pubkey::find_program_address(&seeds, &DELEGATION_PROGRAM_ID);
        assert_eq!(pda, expected.0);
        assert_eq!(bump, expected.1);
    }

    #[test]
    fn test_delegation_record_pda_from_pubkey() {
        let id = Pubkey::new_unique();
        let pda = delegation_record_pda_from_pubkey(&id);
        let seeds = delegation_record_seeds_from_pubkey(&id);
        let expected = Pubkey::find_program_address(&seeds, &DELEGATION_PROGRAM_ID).0;
        assert_eq!(pda, expected);
    }

    #[test]
    fn test_delegation_record_pda_with_bump_from_pubkey() {
        let id = Pubkey::new_unique();
        let (pda, bump) = delegation_record_pda_with_bump_from_pubkey(&id);
        let seeds = delegation_record_seeds_from_pubkey(&id);
        let expected = Pubkey::find_program_address(&seeds, &DELEGATION_PROGRAM_ID);
        assert_eq!(pda, expected.0);
        assert_eq!(bump, expected.1);
    }

    // NOTE: left out remaining checks since they all are implemented via the same macro

    #[test]
    fn test_known_delegation_record() {
        let delegated_addr = "8k2V7EzQtNg38Gi9HK5ZtQYp1YpGKNGrMcuGa737gZX4";
        let delegated_id = Pubkey::from_str(delegated_addr).unwrap();

        let delegation_record_addr = "CkieZJmrj6dLhwteG69LSutpWwRHiDJY9S8ua7xJ7CRW";
        let delegation_record_id = Pubkey::from_str(delegation_record_addr).unwrap();

        let committed_state_addr = "BUrsNkRnqoWUJdGotRt1odFp2NH5b9tcciXzXNbNwBHr";
        let committed_state_id = Pubkey::from_str(committed_state_addr).unwrap();

        let commit_record_addr = "GiDjQqUKeKJwLH5kdbnCgFS2XPGAVjXo73JMoeVn3UZL";
        let commit_record_id = Pubkey::from_str(commit_record_addr).unwrap();

        let delegation_record_pda = delegation_record_pda_from_pubkey(&delegated_id);
        let committed_state_pda = committed_state_pda_from_pubkey(&delegated_id);
        let commit_record_pda = committed_state_record_pda_from_pubkey(&delegated_id);

        assert_eq!(delegation_record_pda, delegation_record_id);
        assert_eq!(committed_state_pda, committed_state_id);
        assert_eq!(commit_record_pda, commit_record_id);
    }
}
