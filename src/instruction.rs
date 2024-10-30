use borsh::{BorshDeserialize, BorshSerialize};
use num_enum::TryFromPrimitive;
use solana_program::program_error::ProgramError;
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    system_program,
};

use crate::consts::{BUFFER, FEES_VAULT};
use crate::pda::{
    buffer_pda_from_pubkey, committed_state_pda_from_pubkey,
    committed_state_record_pda_from_pubkey, delegation_metadata_pda_from_pubkey,
    delegation_record_pda_from_pubkey, ephemeral_balance_pda_from_pubkey,
    validator_fees_vault_pda_from_pubkey,
};

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct DelegateAccountArgs {
    pub valid_until: i64,
    pub commit_frequency_ms: u32,
    pub seeds: Vec<Vec<u8>>,
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct CommitAccountArgs {
    pub slot: u64,
    pub lamports: u64,
    pub allow_undelegation: bool,
    pub data: Vec<u8>,
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct TopUpEphemeralArgs {
    pub amount: u64,
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct WithdrawArgs {
    pub amount: Option<u64>,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, TryFromPrimitive)]
#[rustfmt::skip]
pub enum DlpInstruction {
    Delegate = 0,
    CommitState = 1,
    Finalize = 2,
    Undelegate = 3,
    AllowUndelegate = 4,
    TopUpEphemeralBalance = 5,
    InitFeesVault = 6,
    InitValidatorFeesVault = 7,
    WithdrawEphemeralBalance = 8,
}

impl DlpInstruction {
    pub fn to_vec(&self) -> Vec<u8> {
        let num = *self as u64;
        num.to_le_bytes().to_vec()
    }
}

impl TryFrom<[u8; 8]> for DlpInstruction {
    type Error = ProgramError;
    fn try_from(bytes: [u8; 8]) -> Result<Self, Self::Error> {
        match bytes {
            [0x0, 0, 0, 0, 0, 0, 0, 0] => Ok(DlpInstruction::Delegate),
            [0x1, 0, 0, 0, 0, 0, 0, 0] => Ok(DlpInstruction::CommitState),
            [0x2, 0, 0, 0, 0, 0, 0, 0] => Ok(DlpInstruction::Finalize),
            [0x3, 0, 0, 0, 0, 0, 0, 0] => Ok(DlpInstruction::Undelegate),
            [0x4, 0, 0, 0, 0, 0, 0, 0] => Ok(DlpInstruction::AllowUndelegate),
            [0x5, 0, 0, 0, 0, 0, 0, 0] => Ok(DlpInstruction::TopUpEphemeralBalance),
            [0x6, 0, 0, 0, 0, 0, 0, 0] => Ok(DlpInstruction::InitFeesVault),
            [0x7, 0, 0, 0, 0, 0, 0, 0] => Ok(DlpInstruction::InitValidatorFeesVault),
            [0x8, 0, 0, 0, 0, 0, 0, 0] => Ok(DlpInstruction::WithdrawEphemeralBalance),
            _ => Err(ProgramError::InvalidInstructionData),
        }
    }
}

/// Builds a delegate instruction.
pub fn delegate_from_wrapper_program(
    payer: Pubkey,
    delegate_account: Pubkey,
    system_program: Pubkey,
    delegation_program: Pubkey,
    owner_program: Pubkey,
    discriminator: Vec<u8>,
) -> Instruction {
    let buffer =
        Pubkey::find_program_address(&[BUFFER, &delegate_account.to_bytes()], &owner_program);
    let delegation_record = delegation_record_pda_from_pubkey(&delegate_account);
    let delegate_accounts_seeds = delegation_metadata_pda_from_pubkey(&delegate_account);

    Instruction {
        program_id: owner_program,
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(delegate_account, false),
            AccountMeta::new_readonly(owner_program, false),
            AccountMeta::new(buffer.0, false),
            AccountMeta::new(delegation_record, false),
            AccountMeta::new(delegate_accounts_seeds, false),
            AccountMeta::new_readonly(delegation_program, false),
            AccountMeta::new_readonly(system_program, false),
        ],
        data: discriminator,
    }
}

/// Builds a delegate instruction for an on-curve account.
pub fn delegate_on_curve(
    payer: Pubkey,
    delegate_account: Pubkey,
    system_program: Pubkey,
    args: DelegateAccountArgs,
) -> Instruction {
    let buffer = Pubkey::find_program_address(
        &[BUFFER, &delegate_account.to_bytes()],
        &system_program::id(),
    );
    let delegation_record = delegation_record_pda_from_pubkey(&delegate_account);
    let delegate_accounts_seeds = delegation_metadata_pda_from_pubkey(&delegate_account);
    let mut data = DlpInstruction::Delegate.to_vec();
    data.extend_from_slice(&args.try_to_vec().unwrap());

    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(delegate_account, true),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new(buffer.0, false),
            AccountMeta::new(delegation_record, false),
            AccountMeta::new(delegate_accounts_seeds, false),
            AccountMeta::new_readonly(system_program, false),
        ],
        data,
    }
}
/// Builds a commit state instruction.
pub fn commit_state(
    validator: Pubkey,
    delegated_account: Pubkey,
    commit_args: CommitAccountArgs,
) -> Instruction {
    let commit_args = commit_args.try_to_vec().unwrap();
    let delegation_record_pda = delegation_record_pda_from_pubkey(&delegated_account);
    let commit_state_pda = committed_state_pda_from_pubkey(&delegated_account);
    let commit_state_record_pda = committed_state_record_pda_from_pubkey(&delegated_account);
    let validator_fees_vault_pda = validator_fees_vault_pda_from_pubkey(&validator);
    let delegation_metadata_pda = delegation_metadata_pda_from_pubkey(&delegated_account);
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new_readonly(validator, true),
            AccountMeta::new_readonly(delegated_account, false),
            AccountMeta::new(commit_state_pda, false),
            AccountMeta::new(commit_state_record_pda, false),
            AccountMeta::new(delegation_record_pda, false),
            AccountMeta::new(delegation_metadata_pda, false),
            AccountMeta::new(validator_fees_vault_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: [DlpInstruction::CommitState.to_vec(), commit_args].concat(),
    }
}

/// Builds a finalize state instruction.
pub fn finalize(payer: Pubkey, delegated_account: Pubkey, reimbursement: Pubkey) -> Instruction {
    let delegation_record_pda = delegation_record_pda_from_pubkey(&delegated_account);
    let delegation_metadata_pda = delegation_metadata_pda_from_pubkey(&delegated_account);
    let commit_state_pda = committed_state_pda_from_pubkey(&delegated_account);
    let commit_state_record_pda = committed_state_record_pda_from_pubkey(&delegated_account);
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new_readonly(payer, true),
            AccountMeta::new(delegated_account, false),
            AccountMeta::new(commit_state_pda, false),
            AccountMeta::new(commit_state_record_pda, false),
            AccountMeta::new(delegation_record_pda, false),
            AccountMeta::new(delegation_metadata_pda, false),
            AccountMeta::new(reimbursement, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: DlpInstruction::Finalize.to_vec(),
    }
}

/// Builds an allow_undelegate account instruction.
pub fn allow_undelegate(
    delegated_account: Pubkey,
    owner_program: Pubkey,
    discriminator: Vec<u8>,
) -> Instruction {
    let delegation_record = delegation_record_pda_from_pubkey(&delegated_account);
    let delegation_metadata = delegation_metadata_pda_from_pubkey(&delegated_account);
    let buffer =
        Pubkey::find_program_address(&[BUFFER, &delegated_account.to_bytes()], &owner_program).0;
    Instruction {
        program_id: owner_program,
        accounts: vec![
            AccountMeta::new_readonly(delegated_account, false),
            AccountMeta::new_readonly(delegation_record, false),
            AccountMeta::new(delegation_metadata, false),
            AccountMeta::new_readonly(buffer, false),
            AccountMeta::new_readonly(crate::id(), false),
        ],
        data: discriminator,
    }
}

/// Builds a commit state instruction.
#[allow(clippy::too_many_arguments)]
pub fn undelegate(
    payer: Pubkey,
    delegated_account: Pubkey,
    owner_program: Pubkey,
    reimbursement: Pubkey,
) -> Instruction {
    let delegation_record_pda = delegation_record_pda_from_pubkey(&delegated_account);
    let commit_state_pda = committed_state_pda_from_pubkey(&delegated_account);
    let commit_state_record_pda = committed_state_record_pda_from_pubkey(&delegated_account);
    let delegation_metadata = delegation_metadata_pda_from_pubkey(&delegated_account);
    let buffer_pda = buffer_pda_from_pubkey(&delegated_account);
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(delegated_account, false),
            AccountMeta::new_readonly(owner_program, false),
            AccountMeta::new(buffer_pda, false),
            AccountMeta::new(commit_state_pda, false),
            AccountMeta::new(commit_state_record_pda, false),
            AccountMeta::new(delegation_record_pda, false),
            AccountMeta::new(delegation_metadata, false),
            AccountMeta::new(reimbursement, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: DlpInstruction::Undelegate.to_vec(),
    }
}

/// Initialize a validator fees vault PDA.
pub fn initialize_validator_fees_vault(
    payer: Pubkey,
    admin: Pubkey,
    validator_identity: Pubkey,
) -> Instruction {
    let validator_fees_vault_pda = validator_fees_vault_pda_from_pubkey(&validator_identity);
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(admin, true),
            AccountMeta::new(validator_identity, false),
            AccountMeta::new(validator_fees_vault_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: DlpInstruction::InitValidatorFeesVault.to_vec(),
    }
}

/// Initialize the fees vault PDA.
pub fn initialize_fees_vault(payer: Pubkey) -> Instruction {
    let fees_vault = Pubkey::find_program_address(&[FEES_VAULT], &crate::id()).0;
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(fees_vault, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: DlpInstruction::InitFeesVault.to_vec(),
    }
}

/// Builds a top-up ephemeral balance instruction.
pub fn top_up_ephemeral_balance(payer: Pubkey, amount: u64) -> Instruction {
    let args = TopUpEphemeralArgs { amount };
    let receipt_pda = ephemeral_balance_pda_from_pubkey(&payer);
    let fees_vault = Pubkey::find_program_address(&[FEES_VAULT], &crate::id()).0;
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(receipt_pda, false),
            AccountMeta::new(fees_vault, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: [
            DlpInstruction::TopUpEphemeralBalance.to_vec(),
            args.try_to_vec().unwrap(),
        ]
        .concat(),
    }
}

/// Withdraw ephemeral balance instruction.
pub fn withdraw_ephemeral_balance(payer: Pubkey, amount: Option<u64>) -> Instruction {
    let args = WithdrawArgs { amount };
    let ephemeral_balance_pda = ephemeral_balance_pda_from_pubkey(&payer);
    let fees_vault = Pubkey::find_program_address(&[FEES_VAULT], &crate::id()).0;
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(ephemeral_balance_pda, false),
            AccountMeta::new(fees_vault, false),
        ],
        data: [
            DlpInstruction::WithdrawEphemeralBalance.to_vec(),
            args.try_to_vec().unwrap(),
        ]
        .concat(),
    }
}
