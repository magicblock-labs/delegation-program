use solana_program::pubkey::Pubkey;

pub const DELEGATION_RECORD_SEEDS_PREFIX: &[u8] = b"delegation";
pub const DELEGATION_METADATA_SEEDS_PREFIX: &[u8] = b"delegation-metadata";
pub const COMMIT_STATE_SEEDS_PREFIX: &[u8] = b"state-diff";
pub const COMMIT_RECORD_SEEDS_PREFIX: &[u8] = b"commit-state-record";
pub const FEES_VAULT_SEEDS_PREFIX: &[u8] = b"fees-vault";
pub const VALIDATOR_FEES_VAULT_SEEDS_PREFIX: &[u8] = b"v-fees-vault";
pub const PROGRAM_CONFIG_SEEDS_PREFIX: &[u8] = b"p-conf";
pub const EPHEMERAL_BALANCE_SEEDS_PREFIX: &[u8] = b"balance";

#[macro_export]
macro_rules! delegation_record_seeds_from_delegated_account {
    ($delegated_account: expr) => {
        &[
            $crate::pda::DELEGATION_RECORD_SEEDS_PREFIX,
            &$delegated_account.to_bytes(),
        ]
    };
}

#[macro_export]
macro_rules! delegation_metadata_seeds_from_delegated_account {
    ($delegated_account: expr) => {
        &[
            $crate::pda::DELEGATION_METADATA_SEEDS_PREFIX,
            &$delegated_account.to_bytes(),
        ]
    };
}

#[macro_export]
macro_rules! commit_state_seeds_from_delegated_account {
    ($delegated_account: expr) => {
        &[
            $crate::pda::COMMIT_STATE_SEEDS_PREFIX,
            &$delegated_account.to_bytes(),
        ]
    };
}

#[macro_export]
macro_rules! commit_record_seeds_from_delegated_account {
    ($delegated_account: expr) => {
        &[
            $crate::pda::COMMIT_RECORD_SEEDS_PREFIX,
            &$delegated_account.to_bytes(),
        ]
    };
}

#[macro_export]
macro_rules! fees_vault_seeds {
    () => {
        &[$crate::pda::FEES_VAULT_SEEDS_PREFIX]
    };
}

#[macro_export]
macro_rules! validator_fees_vault_seeds_from_validator {
    ($validator: expr) => {
        &[
            $crate::pda::VALIDATOR_FEES_VAULT_SEEDS_PREFIX,
            &$validator.to_bytes(),
        ]
    };
}

#[macro_export]
macro_rules! program_config_seeds_from_program_id {
    ($program_id: expr) => {
        &[
            $crate::pda::PROGRAM_CONFIG_SEEDS_PREFIX,
            &$program_id.to_bytes(),
        ]
    };
}

#[macro_export]
macro_rules! ephemeral_balance_seeds_from_payer {
    ($payer: expr, $index: expr) => {
        &[
            $crate::pda::EPHEMERAL_BALANCE_SEEDS_PREFIX,
            &$payer.to_bytes(),
            &[$index],
        ]
    };
}

pub fn delegation_record_pda_from_delegated_account(delegated_account: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        delegation_record_seeds_from_delegated_account!(delegated_account),
        &crate::id(),
    )
    .0
}

pub fn delegation_metadata_pda_from_delegated_account(delegated_account: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        delegation_metadata_seeds_from_delegated_account!(delegated_account),
        &crate::id(),
    )
    .0
}

pub fn commit_state_pda_from_delegated_account(delegated_account: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        commit_state_seeds_from_delegated_account!(delegated_account),
        &crate::id(),
    )
    .0
}

pub fn commit_record_pda_from_delegated_account(delegated_account: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        commit_record_seeds_from_delegated_account!(delegated_account),
        &crate::id(),
    )
    .0
}

pub fn fees_vault_pda() -> Pubkey {
    Pubkey::find_program_address(fees_vault_seeds!(), &crate::id()).0
}

pub fn validator_fees_vault_pda_from_validator(validator: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        validator_fees_vault_seeds_from_validator!(validator),
        &crate::id(),
    )
    .0
}

pub fn program_config_from_program_id(program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        program_config_seeds_from_program_id!(program_id),
        &crate::id(),
    )
    .0
}

pub fn ephemeral_balance_pda_from_payer(payer: &Pubkey, index: u8) -> Pubkey {
    Pubkey::find_program_address(
        ephemeral_balance_seeds_from_payer!(payer, index),
        &crate::id(),
    )
    .0
}
