use crate::consts::{BUFFER, EPHEMERAL_BALANCE, FEES_VAULT};
use crate::pda::{
    buffer_pda_from_pubkey, committed_state_pda_from_pubkey,
    committed_state_record_pda_from_pubkey, delegation_metadata_pda_from_pubkey,
    delegation_record_pda_from_pubkey, program_config_pda_from_pubkey,
    validator_fees_vault_pda_from_pubkey,
};
use borsh::{BorshDeserialize, BorshSerialize};
use num_enum::TryFromPrimitive;
use solana_program::program_error::ProgramError;
use solana_program::{
    bpf_loader_upgradeable,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    system_program,
};

#[derive(Default, Debug, BorshSerialize, BorshDeserialize)]
pub struct DelegateAccountArgs {
    pub valid_until: i64,
    pub commit_frequency_ms: u32,
    pub seeds: Vec<Vec<u8>>,
    pub validator: Option<Pubkey>,
}

#[derive(Default, Debug, BorshSerialize, BorshDeserialize)]
pub struct DelegateTopUpAccountArgs {
    pub delegate_args: DelegateAccountArgs,
    pub index: u8,
}

#[derive(Default, Debug, BorshSerialize, BorshDeserialize)]
pub struct CommitAccountArgs {
    pub slot: u64,
    pub lamports: u64,
    pub allow_undelegation: bool,
    pub data: Vec<u8>,
}

#[derive(Default, Debug, BorshSerialize, BorshDeserialize)]
pub struct ClaimFeesArgs {
    pub amount: Option<u64>,
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct TopUpEphemeralArgs {
    pub amount: u64,
    pub index: u8,
}

impl Default for TopUpEphemeralArgs {
    fn default() -> Self {
        Self {
            amount: 10000,
            index: 0,
        }
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct WhitelistValidatorForProgramArgs {
    pub insert: bool,
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
    InitFeesVault = 5,
    InitValidatorFeesVault = 6,
    ValidatorClaimFees = 7,
    WhitelistValidatorForProgram = 8,
    TopUp = 9,
    DelegateEphemeralBalance = 10,
    CloseEphemeralBalance = 11
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
        match bytes[0] {
            0x0 => Ok(DlpInstruction::Delegate),
            0x1 => Ok(DlpInstruction::CommitState),
            0x2 => Ok(DlpInstruction::Finalize),
            0x3 => Ok(DlpInstruction::Undelegate),
            0x4 => Ok(DlpInstruction::AllowUndelegate),
            0x5 => Ok(DlpInstruction::InitFeesVault),
            0x6 => Ok(DlpInstruction::InitValidatorFeesVault),
            0x7 => Ok(DlpInstruction::ValidatorClaimFees),
            0x8 => Ok(DlpInstruction::WhitelistValidatorForProgram),
            0x9 => Ok(DlpInstruction::TopUp),
            0xa => Ok(DlpInstruction::DelegateEphemeralBalance),
            0xb => Ok(DlpInstruction::CloseEphemeralBalance),
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
    let delegate_account_metadata = delegation_metadata_pda_from_pubkey(&delegate_account);

    Instruction {
        program_id: owner_program,
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(buffer.0, false),
            AccountMeta::new(delegation_record, false),
            AccountMeta::new(delegate_account_metadata, false),
            AccountMeta::new(delegate_account, false),
            AccountMeta::new_readonly(owner_program, false),
            AccountMeta::new_readonly(delegation_program, false),
            AccountMeta::new_readonly(system_program, false),
        ],
        data: discriminator,
    }
}

/// Builds a delegate instruction
pub fn delegate(
    payer: Pubkey,
    delegate_account: Pubkey,
    owner: Option<Pubkey>,
    args: DelegateAccountArgs,
) -> Instruction {
    let owner = owner.unwrap_or(system_program::id());
    let buffer = Pubkey::find_program_address(&[BUFFER, &delegate_account.to_bytes()], &owner);
    let delegation_record = delegation_record_pda_from_pubkey(&delegate_account);
    let delegate_account_metadata = delegation_metadata_pda_from_pubkey(&delegate_account);
    let mut data = DlpInstruction::Delegate.to_vec();
    data.extend_from_slice(&args.try_to_vec().unwrap());

    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(delegate_account, true),
            AccountMeta::new_readonly(owner, false),
            AccountMeta::new(buffer.0, false),
            AccountMeta::new(delegation_record, false),
            AccountMeta::new(delegate_account_metadata, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    }
}

/// Delegate ephemeral balance
pub fn delegate_ephemeral_balance(payer: Pubkey, args: DelegateTopUpAccountArgs) -> Instruction {
    let delegate_account = Pubkey::find_program_address(
        &[EPHEMERAL_BALANCE, &payer.to_bytes(), &[args.index]],
        &crate::id(),
    )
    .0;
    let buffer =
        Pubkey::find_program_address(&[BUFFER, &delegate_account.to_bytes()], &crate::id());
    let delegation_record = delegation_record_pda_from_pubkey(&delegate_account);
    let delegate_accounts_metadata = delegation_metadata_pda_from_pubkey(&delegate_account);
    let mut data = DlpInstruction::DelegateEphemeralBalance.to_vec();
    data.extend_from_slice(&args.try_to_vec().unwrap());

    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(delegate_account, false),
            AccountMeta::new(buffer.0, false),
            AccountMeta::new(delegation_record, false),
            AccountMeta::new(delegate_accounts_metadata, false),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(crate::id(), false),
        ],
        data,
    }
}

/// Builds a commit state instruction.
pub fn commit_state(
    validator: Pubkey,
    delegated_account: Pubkey,
    delegated_account_owner: Pubkey,
    commit_args: CommitAccountArgs,
) -> Instruction {
    let commit_args = commit_args.try_to_vec().unwrap();
    let delegation_record_pda = delegation_record_pda_from_pubkey(&delegated_account);
    let commit_state_pda = committed_state_pda_from_pubkey(&delegated_account);
    let commit_state_record_pda = committed_state_record_pda_from_pubkey(&delegated_account);
    let validator_fees_vault_pda = validator_fees_vault_pda_from_pubkey(&validator);
    let delegation_metadata_pda = delegation_metadata_pda_from_pubkey(&delegated_account);
    let whitelist_program_config = program_config_pda_from_pubkey(&delegated_account_owner);
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
            AccountMeta::new_readonly(whitelist_program_config, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: [DlpInstruction::CommitState.to_vec(), commit_args].concat(),
    }
}

/// Builds a finalize state instruction.
pub fn finalize(validator: Pubkey, delegated_account: Pubkey) -> Instruction {
    let delegation_record_pda = delegation_record_pda_from_pubkey(&delegated_account);
    let delegation_metadata_pda = delegation_metadata_pda_from_pubkey(&delegated_account);
    let commit_state_pda = committed_state_pda_from_pubkey(&delegated_account);
    let validator_fees_vault_pda = validator_fees_vault_pda_from_pubkey(&validator);
    let commit_state_record_pda = committed_state_record_pda_from_pubkey(&delegated_account);
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new_readonly(validator, true),
            AccountMeta::new(delegated_account, false),
            AccountMeta::new(commit_state_pda, false),
            AccountMeta::new(commit_state_record_pda, false),
            AccountMeta::new(delegation_record_pda, false),
            AccountMeta::new(delegation_metadata_pda, false),
            AccountMeta::new(validator_fees_vault_pda, false),
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
    validator: Pubkey,
    delegated_account: Pubkey,
    owner_program: Pubkey,
    rent_reimbursement: Pubkey,
) -> Instruction {
    let delegation_record_pda = delegation_record_pda_from_pubkey(&delegated_account);
    let commit_state_pda = committed_state_pda_from_pubkey(&delegated_account);
    let commit_state_record_pda = committed_state_record_pda_from_pubkey(&delegated_account);
    let delegation_metadata = delegation_metadata_pda_from_pubkey(&delegated_account);
    let buffer_pda = buffer_pda_from_pubkey(&delegated_account);
    let validator_fees_vault_pda = validator_fees_vault_pda_from_pubkey(&validator);
    let fees_vault_pda = Pubkey::find_program_address(&[FEES_VAULT], &crate::id()).0;
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(validator, true),
            AccountMeta::new(delegated_account, false),
            AccountMeta::new_readonly(owner_program, false),
            AccountMeta::new(buffer_pda, false),
            AccountMeta::new(commit_state_pda, false),
            AccountMeta::new(commit_state_record_pda, false),
            AccountMeta::new(delegation_record_pda, false),
            AccountMeta::new(delegation_metadata, false),
            AccountMeta::new(rent_reimbursement, false),
            AccountMeta::new(fees_vault_pda, false),
            AccountMeta::new(validator_fees_vault_pda, false),
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

/// Claim the accrued fees from the fees vault.
pub fn validator_claim_fees(validator: Pubkey, amount: Option<u64>) -> Instruction {
    let args = ClaimFeesArgs { amount };
    let fees_vault = Pubkey::find_program_address(&[FEES_VAULT], &crate::id()).0;
    let validator_fees_vault = validator_fees_vault_pda_from_pubkey(&validator);
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(validator, true),
            AccountMeta::new(fees_vault, false),
            AccountMeta::new(validator_fees_vault, false),
        ],
        data: [
            DlpInstruction::ValidatorClaimFees.to_vec(),
            args.try_to_vec().unwrap(),
        ]
        .concat(),
    }
}

/// Whitelist validator for program
pub fn whitelist_validator_for_program(
    authority: Pubkey,
    validator_identity: Pubkey,
    program: Pubkey,
    insert: bool,
) -> Instruction {
    let args = WhitelistValidatorForProgramArgs { insert };
    let program_data =
        Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::id()).0;
    let program_config = program_config_pda_from_pubkey(&program);
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(authority, true),
            AccountMeta::new(validator_identity, false),
            AccountMeta::new_readonly(program, false),
            AccountMeta::new_readonly(program_data, false),
            AccountMeta::new(program_config, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: [
            DlpInstruction::WhitelistValidatorForProgram.to_vec(),
            args.try_to_vec().unwrap(),
        ]
        .concat(),
    }
}

/// Builds a top-up ephemeral balance instruction.
pub fn top_up_ephemeral_balance(
    payer: Pubkey,
    amount: Option<u64>,
    index: Option<u8>,
) -> Instruction {
    let mut args = TopUpEphemeralArgs::default();
    if let Some(amount) = amount {
        args.amount = amount;
    }
    if let Some(index) = index {
        args.index = index;
    }
    let ephemeral_balance = Pubkey::find_program_address(
        &[EPHEMERAL_BALANCE, &payer.to_bytes(), &[args.index]],
        &crate::id(),
    )
    .0;
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(ephemeral_balance, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: [DlpInstruction::TopUp.to_vec(), args.try_to_vec().unwrap()].concat(),
    }
}

pub fn close_ephemeral_balance(payer: Pubkey, index: u8) -> Instruction {
    let ephemeral_balance = Pubkey::find_program_address(
        &[EPHEMERAL_BALANCE, &payer.to_bytes(), &[index]],
        &crate::id(),
    )
    .0;
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(ephemeral_balance, false),
        ],
        data: [DlpInstruction::CloseEphemeralBalance.to_vec(), vec![index]].concat(),
    }
}
