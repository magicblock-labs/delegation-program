use solana_program::{instruction::AccountMeta, pubkey::Pubkey, system_program};

use crate::{
    consts::{BUFFER, DELEGATION_PROGRAM_ID},
    pda::{delegation_metadata_pda_from_pubkey, delegation_record_pda_from_pubkey},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelegateAccounts {
    pub delegate_account: Pubkey,
    pub buffer: Pubkey,
    pub delegation_record: Pubkey,
    pub delegation_metadata: Pubkey,
    pub owner_program: Pubkey,
    pub delegation_program: Pubkey,
    pub system_program: Pubkey,
}

impl DelegateAccounts {
    pub fn new(delegate_account: Pubkey, owner_program: Pubkey) -> Self {
        let buffer =
            Pubkey::find_program_address(&[BUFFER, &delegate_account.to_bytes()], &owner_program);
        let delegation_record = delegation_record_pda_from_pubkey(&delegate_account);
        let delegation_metadata = delegation_metadata_pda_from_pubkey(&delegate_account);

        Self {
            delegate_account,
            buffer: buffer.0,
            delegation_record,
            delegation_metadata,
            owner_program,
            delegation_program: DELEGATION_PROGRAM_ID,
            system_program: system_program::id(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelegateAccountMetas {
    pub payer: AccountMeta,
    pub delegate_account: AccountMeta,
    pub owner_program: AccountMeta,
    pub buffer: AccountMeta,
    pub delegation_record: AccountMeta,
    pub delegation_metadata: AccountMeta,
    pub delegation_program: AccountMeta,
    pub system_program: AccountMeta,
}

impl From<DelegateAccounts> for DelegateAccountMetas {
    fn from(accounts: DelegateAccounts) -> Self {
        Self {
            payer: AccountMeta::new_readonly(accounts.delegate_account, false),
            delegate_account: AccountMeta::new(accounts.delegate_account, false),
            owner_program: AccountMeta::new_readonly(accounts.owner_program, false),
            buffer: AccountMeta::new(accounts.buffer, false),
            delegation_record: AccountMeta::new(accounts.delegation_record, false),
            delegation_metadata: AccountMeta::new(accounts.delegation_metadata, false),
            delegation_program: AccountMeta::new_readonly(accounts.delegation_program, false),
            system_program: AccountMeta::new_readonly(accounts.system_program, false),
        }
    }
}
