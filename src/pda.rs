use solana_program::pubkey::Pubkey;

pub const DELEGATION_RECORD_TAG: &[u8] = b"delegation";
#[macro_export]
macro_rules! delegation_record_seeds_from_delegated_account {
    ($delegated_account: expr) => {
        &[
            $crate::pda::DELEGATION_RECORD_TAG,
            &$delegated_account.as_ref(),
        ]
    };
}

pub const DELEGATION_METADATA_TAG: &[u8] = b"delegation-metadata";
#[macro_export]
macro_rules! delegation_metadata_seeds_from_delegated_account {
    ($delegated_account: expr) => {
        &[
            $crate::pda::DELEGATION_METADATA_TAG,
            &$delegated_account.as_ref(),
        ]
    };
}

pub const COMMIT_STATE_TAG: &[u8] = b"state-diff";
#[macro_export]
macro_rules! commit_state_seeds_from_delegated_account {
    ($delegated_account: expr) => {
        &[$crate::pda::COMMIT_STATE_TAG, &$delegated_account.as_ref()]
    };
}

pub const COMMIT_RECORD_TAG: &[u8] = b"commit-state-record";
#[macro_export]
macro_rules! commit_record_seeds_from_delegated_account {
    ($delegated_account: expr) => {
        &[$crate::pda::COMMIT_RECORD_TAG, &$delegated_account.as_ref()]
    };
}

pub const DELEGATE_BUFFER_TAG: &[u8] = b"buffer";
#[macro_export]
macro_rules! delegate_buffer_seeds_from_delegated_account {
    ($delegated_account: expr) => {
        &[
            $crate::pda::DELEGATE_BUFFER_TAG,
            &$delegated_account.as_ref(),
        ]
    };
}

pub const UNDELEGATE_BUFFER_TAG: &[u8] = b"undelegate-buffer";
#[macro_export]
macro_rules! undelegate_buffer_seeds_from_delegated_account {
    ($delegated_account: expr) => {
        &[
            $crate::pda::UNDELEGATE_BUFFER_TAG,
            &$delegated_account.as_ref(),
        ]
    };
}

#[macro_export]
macro_rules! fees_vault_seeds {
    () => {
        &[b"fees-vault"]
    };
}

pub const VALIDATOR_FEES_VAULT_TAG: &[u8] = b"v-fees-vault";
#[macro_export]
macro_rules! validator_fees_vault_seeds_from_validator {
    ($validator: expr) => {
        &[$crate::pda::VALIDATOR_FEES_VAULT_TAG, &$validator.as_ref()]
    };
}

pub const PROGRAM_CONFIG_TAG: &[u8] = b"p-conf";
#[macro_export]
macro_rules! program_config_seeds_from_program_id {
    ($program_id: expr) => {
        &[$crate::pda::PROGRAM_CONFIG_TAG, &$program_id.as_ref()]
    };
}

pub const EPHEMERAL_BALANCE_TAG: &[u8] = b"balance";
#[macro_export]
macro_rules! ephemeral_balance_seeds_from_payer {
    ($payer: expr, $index: expr) => {
        &[
            $crate::pda::EPHEMERAL_BALANCE_TAG,
            &$payer.as_ref(),
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

pub fn delegate_buffer_pda_from_delegated_account_and_owner_program(
    delegated_account: &Pubkey,
    owner_program: &Pubkey,
) -> Pubkey {
    Pubkey::find_program_address(
        delegate_buffer_seeds_from_delegated_account!(delegated_account),
        owner_program,
    )
    .0
}

pub fn undelegate_buffer_pda_from_delegated_account(delegated_account: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        undelegate_buffer_seeds_from_delegated_account!(delegated_account),
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
